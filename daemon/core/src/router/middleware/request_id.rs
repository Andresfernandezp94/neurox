//! EP-0012 O-002: X-Request-Id middleware.
//!
//! Reads `X-Request-Id` from the request headers, generates a UUID if
//! absent, and creates a `tracing::info_span!("http_request", ...)`
//! scoped to the request. The response carries `X-Request-Id` back
//! to the client so they can correlate logs.
//!
//! This middleware runs BEFORE `JwtAuthLayer` so all requests get an
//! ID, including 401s and public endpoints.

use axum::{
    extract::Request,
    http::{HeaderName, HeaderValue},
    middleware::Next,
    response::Response,
};
use tracing::Instrument;
use uuid::Uuid;

const REQUEST_ID_HEADER: &str = "x-request-id";

pub async fn request_id_layer(mut req: Request, next: Next) -> Response {
    // Extract or generate the request ID.
    let request_id = req
        .headers()
        .get(REQUEST_ID_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    // Inject into request extensions for downstream handlers.
    req.extensions_mut().insert(RequestId(request_id.clone()));

    let method = req.method().clone();
    let uri = req.uri().clone();

    // Wrap the rest of the request in a tracing span.
    let span = tracing::info_span!(
        "http_request",
        request_id = %request_id,
        method = %method,
        path = %uri.path(),
    );

    let mut response = next.run(req).instrument(span).await;

    // Echo the request ID in the response so the client can correlate.
    if let Ok(value) = HeaderValue::from_str(&request_id) {
        response.headers_mut().insert(
            HeaderName::from_static(REQUEST_ID_HEADER),
            value,
        );
    }
    response
}

/// Marker type stored in request extensions to access the request_id.
#[derive(Debug, Clone)]
pub struct RequestId(pub String);

impl RequestId {
    pub fn get(extensions: &axum::http::Extensions) -> Option<&str> {
        extensions.get::<RequestId>().map(|r| r.0.as_str())
    }
}
