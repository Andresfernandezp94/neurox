use axum::extract::Request;
use axum::http::header::AUTHORIZATION;
use axum::http::StatusCode;
use axum::response::Response;
use futures::future::BoxFuture;
use std::task::{Context, Poll};
use subtle::ConstantTimeEq;
use tower::{Layer, Service};

/// Tower layer that enforces Bearer token authentication.
/// If `token` is None, all requests pass through (auth disabled).
#[derive(Clone)]
pub struct AuthLayer {
    token: Option<String>,
}

impl AuthLayer {
    #[must_use]
    pub fn new(token: Option<String>) -> Self {
        Self { token }
    }
}

impl<S> Layer<S> for AuthLayer {
    type Service = AuthService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        AuthService {
            inner,
            token: self.token.clone(),
        }
    }
}

#[derive(Clone)]
pub struct AuthService<S> {
    inner: S,
    token: Option<String>,
}

impl<S> Service<Request> for AuthService<S>
where
    S: Service<Request, Response = Response> + Send + Clone + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let token = if let Some(t) = &self.token {
            t.clone()
        } else {
            let fut = self.inner.call(req);
            return Box::pin(fut);
        };

        let auth_header = req
            .headers()
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "));

        // Constant-time comparison to prevent timing attacks.
        // Only valid when both strings are the same byte length.
        let authorized = match auth_header {
            Some(provided) => {
                let a = provided.as_bytes();
                let b = token.as_bytes();
                a.len() == b.len() && a.ct_eq(b).into()
            }
            None => false,
        };

        if authorized {
            let fut = self.inner.call(req);
            Box::pin(fut)
        } else {
            Box::pin(async {
                Ok(axum::http::Response::builder()
                    .status(StatusCode::UNAUTHORIZED)
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        r#"{"error":"unauthorized","message":"valid Bearer token required"}"#,
                    ))
                    .expect("static response build"))
            })
        }
    }
}
