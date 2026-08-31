// EP-0007 — Auth middleware.
//
// Two pieces:
//   - `JwtAuthLayer`: Tower layer that extracts Bearer JWT, verifies it,
//     and injects `UserContext` into request extensions. Public paths
//     (login, /health, static dir) bypass.
//   - `UserContext` + role-check helpers (`require_admin`, `require_min_role`)
//     for handlers.

use axum::extract::Request;
use axum::http::{header::AUTHORIZATION, StatusCode};
use axum::response::{IntoResponse, Response};
use futures::future::BoxFuture;
use std::sync::Arc;
use std::task::{Context, Poll};
use tower::{Layer, Service};

use super::tokens::{verify_token, JwtSecret};
pub use super::users::Role;

#[derive(Debug, Clone)]
pub struct UserContext {
    pub user_id: uuid::Uuid,
    pub username: String,
    pub role: Role,
}

/// Layer factory. Wrap a router with this to require auth on all paths
/// EXCEPT those in `public_paths`.
#[derive(Clone)]
pub struct JwtAuthLayer {
    secret: Arc<JwtSecret>,
    public_paths: Arc<Vec<&'static str>>,
}

impl JwtAuthLayer {
    pub fn new(secret: Arc<JwtSecret>, public_paths: Vec<&'static str>) -> Self {
        Self {
            secret,
            public_paths: Arc::new(public_paths),
        }
    }
}

impl<S> Layer<S> for JwtAuthLayer {
    type Service = JwtAuthService<S>;
    fn layer(&self, inner: S) -> Self::Service {
        JwtAuthService {
            inner,
            secret: self.secret.clone(),
            public_paths: self.public_paths.clone(),
        }
    }
}

#[derive(Clone)]
pub struct JwtAuthService<S> {
    inner: S,
    secret: Arc<JwtSecret>,
    public_paths: Arc<Vec<&'static str>>,
}

impl<S> Service<Request> for JwtAuthService<S>
where
    S: Service<Request, Response = Response> + Send + Clone + 'static,
    S::Future: Send + 'static,
    S::Error: IntoResponse + Send + 'static,
{
    type Response = Response;
    type Error = S::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request) -> Self::Future {
        let path = req.uri().path().to_string();
        let is_public = self.public_paths.iter().any(|p| path.starts_with(p));
        // EP-0011: loopback connections are NO LONGER bypassed with
        // synthetic Admin role. They must present a valid JWT, like any
        // other peer. The only exceptions are the explicitly-listed
        // public paths (health checks, login, metrics).
        //
        // Plugins running on the same host (memoryd, voiced, etc.) must
        // configure a bearer token via `NEUROX_<NAME>_TOKEN` env vars.
        // See docs/daemon/plugins.md for setup.
        let secret = self.secret.clone();
        let auth_header = req
            .headers()
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        if is_public {
            let fut = self.inner.call(req);
            return Box::pin(fut);
        }

        // Try the Bearer header first; fall back to `?token=` query string
        // for WebSocket upgrades (browsers don't allow custom headers
        // in the WS handshake).
        let token = auth_header
            .as_deref()
            .and_then(extract_bearer_from_str)
            .or_else(|| {
                // Minimal `token=...` extractor (avoids pulling the
                // `url` crate). Handles `?token=...&foo=bar` correctly.
                req.uri().query().and_then(|q| {
                    for pair in q.split('&') {
                        if let Some(rest) = pair.strip_prefix("token=") {
                            return Some(
                                rest.split('&').next().unwrap_or("").to_string(),
                            );
                        }
                    }
                    None
                })
            });

        let result = token.and_then(|t| {
            verify_token(&secret, &t)
                .ok()
                .and_then(|claims| {
                    uuid::Uuid::parse_str(&claims.sub).ok().map(|uid| UserContext {
                        user_id: uid,
                        username: claims.username,
                        role: claims.role,
                    })
                })
        });

        match result {
            Some(ctx) => {
                req.extensions_mut().insert(ctx);
                let fut = self.inner.call(req);
                Box::pin(fut)
            }
            None => Box::pin(async move { Ok(unauthorized_response()) }),
        }
    }
}

fn extract_bearer_from_str(s: &str) -> Option<String> {
    if let Some(rest) = s.strip_prefix("Bearer ") {
        Some(rest.to_string())
    } else {
        None
    }
}

fn unauthorized_response() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        axum::Json(serde_json::json!({
            "error": "unauthorized",
            "message": "valid Bearer token required"
        })),
    )
        .into_response()
}

/// Helper for handlers that want to require an exact role.
pub fn require_admin(req: &Request) -> Result<UserContext, Response> {
    let user = req
        .extensions()
        .get::<UserContext>()
        .cloned()
        .ok_or_else(unauthorized_response)?;
    if user.role == Role::Admin {
        Ok(user)
    } else {
        Err(forbidden_response("Admin"))
    }
}

/// Helper for handlers that want to require role >= `min`.
pub fn require_min_role(req: &Request, min: Role) -> Result<UserContext, Response> {
    let user = req
        .extensions()
        .get::<UserContext>()
        .cloned()
        .ok_or_else(unauthorized_response)?;
    if role_at_least(user.role, min) {
        Ok(user)
    } else {
        Err(forbidden_response(min.as_str()))
    }
}

fn forbidden_response(required: &str) -> Response {
    (
        StatusCode::FORBIDDEN,
        axum::Json(serde_json::json!({
            "error": "forbidden",
            "message": format!("{required} role required for this resource")
        })),
    )
        .into_response()
}

fn role_at_least(actual: Role, min: Role) -> bool {
    fn rank(r: Role) -> u8 {
        match r {
            Role::Viewer => 1,
            Role::Operator => 2,
            Role::Admin => 3,
        }
    }
    rank(actual) >= rank(min)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_ranking() {
        assert!(role_at_least(Role::Admin, Role::Operator));
        assert!(role_at_least(Role::Operator, Role::Viewer));
        assert!(!role_at_least(Role::Viewer, Role::Operator));
        assert!(!role_at_least(Role::Operator, Role::Admin));
    }
}