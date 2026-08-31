//! neurox-web — minimal static-file server for the neurox web UI.
//!
//! Serves the SPA bundle (default: `./dist`) on a single port, with:
//!   - SPA fallback: any GET that doesn't match a file falls back to
//!     `index.html` so client-side routing works on refresh
//!   - Security headers: nosniff, no-referrer, no-embedding
//!   - Brotli/gzip compression for text-ish assets
//!   - Long-lived cache for fingerprinted assets (paths containing `/-/`
//!     or `/@fs/`, or `.[hash].ext` patterns)
//!   - Short cache for `index.html` (force revalidation)
//!
//! The daemon origin (CORS + auth) is exposed via `VITE_API_BASE` at
//! build time, so this server does NOT proxy `/v1/*`. The browser
//! calls the daemon directly using the configured base.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use axum::{
    body::Body,
    extract::{Request, State},
    http::{header, HeaderValue, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use clap::Parser;
use tower_http::{
    compression::CompressionLayer,
    set_header::SetResponseHeaderLayer,
    trace::TraceLayer,
};
use tracing::{info, warn};

#[derive(Parser, Debug)]
#[command(name = "neurox-web", version, about = "Static-file server for the neurox web UI")]
struct Args {
    /// Directory containing the SPA bundle to serve.
    #[arg(long, env = "NEUROX_WEB_DIR", default_value = "./dist")]
    dir: PathBuf,

    /// Bind address.
    #[arg(long, env = "NEUROX_WEB_HOST", default_value = "127.0.0.1")]
    host: String,

    /// Bind port.
    #[arg(long, env = "NEUROX_WEB_PORT", default_value_t = 8787)]
    port: u16,

    /// Enable SPA fallback (rewrite unknown paths to /index.html).
    #[arg(long, env = "NEUROX_WEB_SPA_FALLBACK", default_value_t = true)]
    spa_fallback: bool,

    /// Log filter (RUST_LOG-style).
    #[arg(long, env = "RUST_LOG", default_value = "info,tower_http=info")]
    log: String,
}

#[derive(Clone)]
struct Config {
    dir: Arc<PathBuf>,
    spa_fallback: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    tracing_subscriber::fmt()
        .with_env_filter(args.log.clone())
        .with_target(false)
        .compact()
        .init();

    let dir = args
        .dir
        .canonicalize()
        .with_context(|| format!("canonicalize dir `{}`", args.dir.display()))?;
    if !dir.is_dir() {
        anyhow::bail!("`{}` is not a directory", dir.display());
    }
    let index = dir.join("index.html");
    if !index.is_file() {
        anyhow::bail!(
            "`{}/index.html` not found — build the SPA first (pnpm build) and pass --dir",
            dir.display()
        );
    }

    let cfg = Config {
        dir: Arc::new(dir.clone()),
        spa_fallback: args.spa_fallback,
    };

    let app = Router::new()
        .route("/health", get(health))
        .fallback(fallback)
        .with_state(cfg.clone())
        .layer(middleware::from_fn(security_headers))
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        // Default permissive CORS — the browser already enforces,
        // and the SPA talks to the daemon via VITE_API_BASE, not us.
        .layer(SetResponseHeaderLayer::if_not_present(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-cache"),
        ));

    let addr: SocketAddr = format!("{}:{}", args.host, args.port)
        .parse()
        .with_context(|| format!("parse {}:{}", args.host, args.port))?;

    info!(%addr, dir = %dir.display(), spa_fallback = args.spa_fallback, "neurox-web listening");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("bind {addr}"))?;
    info!("press Ctrl-C to stop");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("axum::serve")?;
    Ok(())
}

async fn health() -> &'static str {
    "ok"
}

async fn fallback(State(cfg): State<Config>, req: Request<Body>) -> Response {
    let path = req.uri().path();

    // Only handle GET/HEAD — POST/PUT/etc. return 404 (no API here).
    if !matches!(req.method(), &axum::http::Method::GET | &axum::http::Method::HEAD) {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }

    // Reject path traversal: anything containing `..` segment.
    if path.split('/').any(|seg| seg == "..") {
        return (StatusCode::BAD_REQUEST, "bad path").into_response();
    }

    let mut file_path = cfg.dir.join(path.trim_start_matches('/'));

    // If the path is a directory, look for index.html inside it.
    let mut is_file_serve = false;
    if file_path.is_file() {
        is_file_serve = true;
    } else if file_path.is_dir() {
        let index = file_path.join("index.html");
        if index.is_file() {
            file_path = index;
            is_file_serve = true;
        }
    }

    // Serve the file.
    if is_file_serve {
        match tokio::fs::read(&file_path).await {
            Ok(bytes) => {
                let mime = mime_guess::from_path(&file_path).first_or_octet_stream();
                let mut resp = (StatusCode::OK, bytes).into_response();
                resp.headers_mut().insert(
                    header::CONTENT_TYPE,
                    HeaderValue::from_str(mime.essence_str())
                        .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
                );

                // Fingerprinted assets get long cache; everything else revalidates.
                let cache = if is_fingerprinted_asset(path) {
                    "public, max-age=31536000, immutable"
                } else {
                    "no-cache, must-revalidate"
                };
                resp.headers_mut().insert(
                    header::CACHE_CONTROL,
                    HeaderValue::from_static(cache),
                );
                return resp;
            }
            Err(e) => {
                warn!(file = %file_path.display(), error = %e, "read failed");
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        }
    }

    // SPA fallback: serve index.html for any non-asset path.
    if cfg.spa_fallback && !path.contains('.') {
        let index = cfg.dir.join("index.html");
        match tokio::fs::read(&index).await {
            Ok(bytes) => {
                let mut resp = (StatusCode::OK, bytes).into_response();
                resp.headers_mut().insert(
                    header::CONTENT_TYPE,
                    HeaderValue::from_static("text/html; charset=utf-8"),
                );
                resp.headers_mut().insert(
                    header::CACHE_CONTROL,
                    HeaderValue::from_static("no-cache, must-revalidate"),
                );
                return resp;
            }
            Err(e) => {
                warn!(error = %e, "read index.html failed");
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        }
    }

    (StatusCode::NOT_FOUND, "not found").into_response()
}

fn is_fingerprinted_asset(path: &str) -> bool {
    // Vite fingerprinted assets look like:
    //   /assets/index-abc123def.js
    //   /assets/index-abc123def.css
    // heuristic: contains a long hex-looking hash before the extension.
    let last = path.rsplit('/').next().unwrap_or("");
    if let Some(dot) = last.rfind('.') {
        let stem = &last[..dot];
        if let Some(dash) = stem.rfind('-') {
            let hash = &stem[dash + 1..];
            return hash.len() >= 8 && hash.chars().all(|c| c.is_ascii_alphanumeric());
        }
    }
    false
}

async fn security_headers(req: Request<Body>, next: Next) -> Response {
    let mut resp = next.run(req).await;
    let headers = resp.headers_mut();
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    headers.insert(
        "X-Frame-Options",
        HeaderValue::from_static("DENY"),
    );
    headers.insert(
        header::HeaderName::from_static("cross-origin-opener-policy"),
        HeaderValue::from_static("same-origin"),
    );
    resp
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("install Ctrl-C handler");
    };
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => { info!("Ctrl-C received, shutting down"); },
        _ = terminate => { info!("SIGTERM received, shutting down"); },
    }

    // Give in-flight requests a moment to finish.
    tokio::time::sleep(Duration::from_millis(200)).await;
}
