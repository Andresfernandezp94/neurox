//! Shared `reqwest::Client` for the daemon + engine.
//!
//! EP-0004 wave 5a: 19 sites across the codebase each constructed
//! their own `reqwest::Client::builder()` with possibly different
//! timeouts, TLS settings, and no shared connection pool. This module
//! provides a single factory that produces a `Client` with sensible
//! defaults, built once and shared by every HTTP caller.
//!
//! The factory is owned by `tools_engine::Engine` (constructed in
//! `Engine::new`); every code path that needs an HTTP client borrows
//! `Arc<reqwest::Client>` from the engine via `state.engine.http_client()`
//! (the daemon side) or as a constructor argument (the engine-internal
//! tools).

use std::time::Duration;

use reqwest::Client;

/// Tunable parameters for building the shared HTTP client.
///
/// Defaults are conservative (30s total timeout, 10s connect, no
/// proxy, system trust roots). Tests can opt out of cert validation
/// with `accept_invalid_certs = true`.
#[derive(Debug, Clone)]
pub struct HttpClientConfig {
    pub timeout_secs: u64,
    pub connect_timeout_secs: u64,
    pub user_agent: String,
    pub proxy: Option<String>,
    pub accept_invalid_certs: bool,
}

impl Default for HttpClientConfig {
    fn default() -> Self {
        Self {
            timeout_secs: 30,
            connect_timeout_secs: 10,
            user_agent: format!("neurox/{}", env!("CARGO_PKG_VERSION")),
            proxy: None,
            accept_invalid_certs: false,
        }
    }
}

/// Build a `reqwest::Client` from the given config. Errors on invalid
/// configuration (e.g. malformed proxy URL).
pub fn build(cfg: &HttpClientConfig) -> Result<Client, reqwest::Error> {
    let mut builder = Client::builder()
        .timeout(Duration::from_secs(cfg.timeout_secs))
        .connect_timeout(Duration::from_secs(cfg.connect_timeout_secs))
        .user_agent(cfg.user_agent.clone());
    if let Some(proxy) = &cfg.proxy {
        builder = builder.proxy(reqwest::Proxy::all(proxy)?);
    }
    if cfg.accept_invalid_certs {
        builder = builder.danger_accept_invalid_certs(true);
    }
    builder.build()
}