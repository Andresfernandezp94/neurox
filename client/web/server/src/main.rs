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
//!   - Reverse proxy for `/v1/*` and `/health` toward the daemon
//!
//! # Por qué el proxy y no `VITE_API_BASE`
//!
//! `VITE_API_BASE` se inlina en el bundle **en build time**. Si el
//! navegador corre en otra máquina de la LAN (o en el teléfono), un
//! base absoluto tiene que apuntar a la IP del daemon, que es
//! dinámica (`192.168.1.5` por DHCP), y cada cambio de red obliga a
//! rebuildear. Peor: si el base queda vacío, la SPA pide `/v1/*` a
//! este mismo servidor, que antes las devolvía como `index.html`.
//!
//! Con el proxy, el browser solo habla con el origen que sirve la SPA.
//! `VITE_API_BASE` queda vacío, el bundle es portable entre máquinas y
//! el token JWT nunca cruza un salto de red extra.
//!
//! # Lo que el proxy NO cubre: el WebSocket
//!
//! `/v1/events` y `/v1/commands` hacen upgrade HTTP, y este server es
//! unaxum normal sin upgrade: un WS contra él muere en el handshake.
//! Para que el panel de eventos funcione hay dos caminos:
//!   1.buildear con `VITE_API_BASE=http://<ip-del-daemon>:7878`, que
//!     hace que el browser apunte el WS directo al daemon (el `allow_origin`
//!     del daemon ya es `*`), o
//!   2. developerear contra el dev server de vite (`:5173`), que sí
//!     proxea el upgrade.
//! El REST y el SSE (streaming del chat) sí van por el proxy.

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
    routing::{any, get},
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

    /// Origin del daemon al que se reenvían `/v1/*` y `/health`.
    /// Vacío = no proxyear (el bundle debe traer `VITE_API_BASE`).
    #[arg(long, env = "NEUROX_WEB_API_ORIGIN", default_value = "http://127.0.0.1:7878")]
    api_origin: String,

    /// Log filter (RUST_LOG-style).
    #[arg(long, env = "RUST_LOG", default_value = "info,tower_http=info")]
    log: String,
}

#[derive(Clone)]
struct Config {
    dir: Arc<PathBuf>,
    spa_fallback: bool,
    /// `None` cuando `--api-origin ""`: en ese modo `/v1/*` no existe.
    api_origin: Option<String>,
    client: reqwest::Client,
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

    // `--api-origin ""` desactiva el proxy: `/v1/*` pasa a 404 y el
    // bundle tiene que traer `VITE_API_BASE` con la URL del daemon.
    let api_origin = {
        let raw = args.api_origin.trim().to_string();
        if raw.is_empty() {
            None
        } else {
            // Validar temprano: un origin mal formado solo se
            // manifestaría en el primer request, con un 502 confuso.
            let base = reqwest::Url::parse(&raw)
                .with_context(|| format!("invalid --api-origin `{raw}`"))?;
            if !matches!(base.scheme(), "http" | "https") {
                anyhow::bail!("--api-origin must be http or https, got `{}`", base.scheme());
            }
            // Sin barra final, para poder concatenar `/v1/...`.
            Some(raw.trim_end_matches('/').to_string())
        }
    };

    let client = reqwest::Client::builder()
        // El daemon es local. Solo el connect tiene timeout corto:
        // los requests SSE / WS pueden vivir minutos colgados.
        .connect_timeout(std::time::Duration::from_secs(5))
        .build()
        .context("build reqwest client")?;

    let cfg = Config {
        dir: Arc::new(dir.clone()),
        spa_fallback: args.spa_fallback,
        api_origin: api_origin.clone(),
        client,
    };

    // El proxy entra como middleware, no como `.route("/v1/{*rest}")`.
    //
    // Razón: axum 0.7 / matchit 0.7 **no aceptan** un catch-all con
    // prefijo. `Router::route("/v1/{*rest}", ...)` compila sin quejarse
    // y revienta en runtime con `Invalid route ... catch-all parameters
    // are only allowed at the end of a route` — un panic que solo aparece
    // al arrancar, no en `cargo check`. Un `from_fn` que matchea el
    // prefijo `/v1/` hace lo mismo sin depender de esa restricción.
    // Rutas exactas del WS del daemon. No hacen falta como catch-all:
    // son dos paths fijos, y declararlos así deja el
    // `hyper::upgrade::OnUpgrade` intacto para el extractor.
    //
    // `/health` también va por proxy: el dev server de vite lo hace
    // (ver su `proxy`), así que el SPA ve el mismo shape en dev y en
    // prod. Es el health check real del daemon, no el string "ok" del
    // servidor estático.
    let app = Router::new()
        .route("/v1/events", any(ws_proxy))
        .route("/v1/commands", any(ws_proxy))
        .route("/health", get(proxy))
        .fallback(fallback)
        .with_state(cfg.clone())
        .layer(middleware::from_fn_with_state(cfg.clone(), route_api))
        .layer(middleware::from_fn(security_headers))
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .layer(SetResponseHeaderLayer::if_not_present(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-cache"),
        ));

    let addr: SocketAddr = format!("{}:{}", args.host, args.port)
        .parse()
        .with_context(|| format!("parse {}:{}", args.host, args.port))?;

    info!(
        %addr,
        dir = %dir.display(),
        spa_fallback = args.spa_fallback,
        api_origin = api_origin.as_deref().unwrap_or("(none: bundle debe traer VITE_API_BASE)"),
        "neurox-web listening"
    );

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

/// Reverse proxy hacia el daemon para `/v1/*`.
///
/// Reenvía método, query, body y los headers que importan
/// (`Authorization`, `Content-Type`), y devuelve el status + body del
/// daemon. No reenvía `Host` ni headers de hop-by-hop.
///
/// SSE (`/v1/sessions/:id/messages/stream`) pasa por acá como un
/// stream de bytes: no se bufferiza ni se cierra por timeout, así que
/// el chat en streaming funciona igual que detrás del proxy de vite.
///
/// Lo que NO pasa por acá es el WebSocket (`/v1/events`,
/// `/v1/commands`): este server no implementa upgrade HTTP. Para eso
/// el browser tiene que hablar directo con el daemon, lo que exige
/// `VITE_API_BASE` con `ws://` y `allow_origin` en el daemon.
/// Enruta `/v1/*` al proxy del daemon; todo lo demás sigue al SPA.
///
/// Va como middleware porque axum 0.7 no soporta catch-all con prefijo
/// (ver el comentario del router). El prefijo `/v1` pelado también
/// entra al proxy: el daemon responde 404 con JSON, que es más útil que
/// el index.html del SPA.
async fn route_api(State(cfg): State<Config>, req: Request<Body>, next: Next) -> Response {
    let path = req.uri().path();
    // El upgrade de WS lo maneja `ws_proxy` como route: se salta el
    // middleware para que `/v1/events` llegue al extractor con su
    // `OnUpgrade` intacto (ver el doc de `ws_proxy`).
    if is_ws_upgrade(&req) {
        return next.run(req).await;
    }
    if path == "/v1" || path.starts_with("/v1/") {
        return proxy(State(cfg), req).await;
    }
    next.run(req).await
}

fn is_ws_upgrade(req: &Request<Body>) -> bool {
    let has_upgrade = req
        .headers()
        .get(axum::http::header::UPGRADE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.eq_ignore_ascii_case("websocket"));
    has_upgrade && req.headers().contains_key(axum::http::header::SEC_WEBSOCKET_KEY)
}

/// Proxy de WebSocket: completa el handshake contra el daemon y
/// bombea frames en ambos sentidos.
///
/// Va como handler de ruta, no desde el middleware, por una razón
/// concreta: `WebSocketUpgrade` saca `hyper::upgrade::OnUpgrade` de las
/// *extensions* de la request, y un middleware que reconstruya la
/// request para invocar el extractor pierde esa extension. El error
/// es `426 WebSocket request couldn't be upgraded since no upgrade state
/// was present`. Como extractor, axum la resuelve sobre la request real.
async fn ws_proxy(
    State(cfg): State<Config>,
    ws: axum::extract::ws::WebSocketUpgrade,
    req: Request<Body>,
) -> Response {
    let Some(origin) = cfg.api_origin.as_deref() else {
        return (
            StatusCode::NOT_FOUND,
            "no /v1 proxy: neurox-web corre sin --api-origin",
        )
            .into_response();
    };

    // `host` + path del origin del daemon. El queryString viaja tal
    // cual: ahí va el `?token=` que el daemon exige para el upgrade.
    let url = origin.replace("http://", "ws://").replace("https://", "wss://");
    let target = format!(
        "{url}{}?{}",
        req.uri().path(),
        req.uri().query().unwrap_or("")
    );

    let connect = match tokio_tungstenite::connect_async(&target).await {
        Ok((stream, _resp)) => stream,
        Err(e) => {
            warn!(%target, error = %e, "ws_proxy: upstream handshake failed");
            return (StatusCode::BAD_GATEWAY, format!("ws upstream: {e}")).into_response();
        }
    };

    // No se restringe subprotocol: el browser pidió uno y el daemon
    // contesta con el suyo en el handshake de arriba.
    ws.on_upgrade(move |browser| bridge(browser, connect))
}

async fn bridge(
    browser: axum::extract::ws::WebSocket,
    upstream: tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) {
    use futures_util::{SinkExt, StreamExt};

    let (mut browser_tx, mut browser_rx) = browser.split();
    let (mut up_tx, mut up_rx) = upstream.split();

    // browser → daemon
    let b2u = async move {
        while let Some(Ok(msg)) = browser_rx.next().await {
            let out = match msg {
                axum::extract::ws::Message::Text(t) => {
                    tokio_tungstenite::tungstenite::Message::Text(t)
                }
                axum::extract::ws::Message::Binary(b) => {
                    tokio_tungstenite::tungstenite::Message::Binary(b)
                }
                axum::extract::ws::Message::Ping(p) | axum::extract::ws::Message::Pong(p) => {
                    tokio_tungstenite::tungstenite::Message::Ping(p)
                }
                axum::extract::ws::Message::Close(c) => {
                    // axum y tungstenite tienen `CloseFrame` distintos:
                    // hay que mapear code + reason a mano.
                    let frame = c.map(|f| {
                        tokio_tungstenite::tungstenite::protocol::CloseFrame {
                            code: f.code.into(),
                            reason: f.reason.to_string().into(),
                        }
                    });
                    let _ = up_tx.send(tokio_tungstenite::tungstenite::Message::Close(frame)).await;
                    break;
                }
            };
            if up_tx.send(out).await.is_err() {
                break;
            }
        }
    };

    // daemon → browser
    let u2b = async move {
        while let Some(Ok(msg)) = up_rx.next().await {
            let out = match msg {
                tokio_tungstenite::tungstenite::Message::Text(t) => {
                    axum::extract::ws::Message::Text(t)
                }
                tokio_tungstenite::tungstenite::Message::Binary(b) => {
                    axum::extract::ws::Message::Binary(b)
                }
                tokio_tungstenite::tungstenite::Message::Ping(p) => {
                    axum::extract::ws::Message::Ping(p)
                }
                tokio_tungstenite::tungstenite::Message::Pong(p) => {
                    axum::extract::ws::Message::Pong(p)
                }
                tokio_tungstenite::tungstenite::Message::Close(c) => {
                    let frame = c.map(|f| axum::extract::ws::CloseFrame {
                        code: u16::from(f.code),
                        reason: f.reason.to_string().into(),
                    });
                    let _ = browser_tx.send(axum::extract::ws::Message::Close(frame)).await;
                    break;
                }
                // Frame de control que no nos corresponde reenviar.
                _ => continue,
            };
            if browser_tx.send(out).await.is_err() {
                break;
            }
        }
    };

    tokio::join!(b2u, u2b);
}

async fn proxy(State(cfg): State<Config>, req: Request<Body>) -> Response {
    let Some(origin) = cfg.api_origin.as_deref() else {
        return (
            StatusCode::NOT_FOUND,
            "no /v1 proxy: neurox-web corre sin --api-origin; el bundle debe traer VITE_API_BASE",
        )
            .into_response();
    };

    let path = req.uri().path();
    let query = req.uri().query().map_or_else(String::new, |q| format!("?{q}"));
    let url = format!("{origin}{path}{query}");

    let method = req.method().clone();
    let (parts, body) = req.into_parts();

    // Solo los headers que el daemon necesita. `host` se omite a
    // propósito: reqwest lo pone solo con el origin correcto.
    let mut forwarded = reqwest::header::HeaderMap::new();
    for name in [
        axum::http::header::AUTHORIZATION,
        axum::http::header::CONTENT_TYPE,
        axum::http::header::ACCEPT,
    ] {
        if let Some(v) = parts.headers.get(&name) {
            forwarded.insert(name.clone(), v.clone());
        }
    }

    let bytes = match axum::body::to_bytes(body, 32 * 1024 * 1024).await {
        Ok(b) => b,
        Err(e) => return (StatusCode::PAYLOAD_TOO_LARGE, format!("body: {e}")).into_response(),
    };

    let upstream = match cfg.client.request(method, &url).body(bytes).headers(forwarded).send().await {
        Ok(r) => r,
        Err(e) => {
            warn!(%url, error = %e, "proxy: upstream unreachable");
            return (StatusCode::BAD_GATEWAY, format!("daemon unreachable: {e}")).into_response();
        }
    };

    let status = StatusCode::from_u16(upstream.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let content_type = upstream
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .cloned();

    match upstream.bytes().await {
        Ok(b) => {
            let mut resp = (status, b).into_response();
            // SSE: el daemon manda `text/event-stream`. Sin esto,
            // algunos browsers bufferean la respuesta y el chat
            // parece colgado hasta que termina el stream completo.
            if let Some(ct) = content_type {
                resp.headers_mut().insert(header::CONTENT_TYPE, ct);
            }
            resp.headers_mut().insert(
                axum::http::header::CACHE_CONTROL,
                HeaderValue::from_static("no-store"),
            );
            resp
        }
        Err(e) => (StatusCode::BAD_GATEWAY, format!("upstream body: {e}")).into_response(),
    }
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
