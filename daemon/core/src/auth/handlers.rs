// EP-0007 — HTTP handlers for /v1/auth/* and /v1/users/*.

use axum::extract::{Extension, FromRequestParts, Path};
use axum::http::request::Parts;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use super::middleware::UserContext;
use super::tokens::{issue_token, JwtSecret};
use super::users::{Role, UserStore, UserStoreError, UserView};

/// Shared state for auth routes.
#[derive(Clone)]
pub struct AuthState {
    pub user_store: Arc<UserStore>,
    pub secret: Arc<JwtSecret>,
    pub expiry_hours: u64,
    /// EP-0014 C-005: re-authentication tokens required for role changes.
    pub reauth_tokens: Arc<ReauthTokens>,
    /// Long-lived refresh tokens. The access JWT stays short-lived; this is
    /// what lets the mobile stay signed in for weeks.
    pub refresh_store: Arc<super::refresh::RefreshStore>,
    pub refresh_ttl_days: u64,
    /// Google Sign-In. `client_id` empty means the provider is not
    /// configured and `/v1/auth/google` answers 404.
    pub google_client_id: String,
    pub google_client_secret: String,
    pub google_audience: String,
}

/// EP-0014 C-005: short-lived tokens for sensitive operations. Each
/// token has a TTL of 5 minutes. `issue()` is called after a fresh
/// login; `validate()` consumes a token (single-use).
pub struct ReauthTokens {
    inner: parking_lot::Mutex<HashMap<String, std::time::Instant>>,
}

impl ReauthTokens {
    pub fn new() -> Self {
        Self {
            inner: parking_lot::Mutex::new(HashMap::new()),
        }
    }

    pub fn issue(&self) -> String {
        let token = uuid::Uuid::new_v4().to_string();
        let expiry = std::time::Instant::now() + std::time::Duration::from_secs(300);
        self.inner.lock().insert(token.clone(), expiry);
        token
    }

    pub fn validate(&self, token: &str) -> bool {
        let mut g = self.inner.lock();
        if let Some(expiry) = g.remove(token) {
            std::time::Instant::now() < expiry
        } else {
            false
        }
    }
}

/// Extractor: 401 si no hay UserContext, 403 si no es Admin.
/// Pasa el `UserContext` al handler para uso posterior.
pub struct RequireAdmin(pub UserContext);

#[axum::async_trait]
impl<S> FromRequestParts<S> for RequireAdmin
where
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let user = parts
            .extensions
            .get::<UserContext>()
            .cloned()
            .ok_or_else(forbidden_or_unauth)?;
        if user.role != Role::Admin {
            return Err(forbidden_json());
        }
        Ok(Self(user))
    }
}

fn forbidden_or_unauth() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(ErrorBody {
            error: "unauthorized",
            message: "valid Bearer token required".into(),
        }),
    )
        .into_response()
}

fn forbidden_json() -> Response {
    (
        StatusCode::FORBIDDEN,
        Json(ErrorBody {
            error: "forbidden",
            message: "Admin role required for this resource".into(),
        }),
    )
        .into_response()
}

// ────────────────────────────────────────────────────────────────────
// Request / response DTOs
// ────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub user: UserView,
    /// EP-0014 C-005: short-lived token for sensitive ops (e.g. role changes).
    pub reauth_token: String,
    /// Long-lived credential to obtain a new `token` without re-entering the
    /// password. `None` when the refresh store could not issue one, so an
    /// old client that ignores it keeps working unchanged.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
}

/// Body of `POST /v1/auth/refresh-token`. Distinct from `/v1/auth/refresh`
/// (which needs a still-valid JWT and only slides the expiry): this one
/// takes the long-lived credential, so it works from a cold start on the
/// mobile without any access token at all.
#[derive(Deserialize)]
pub struct RefreshTokenRequest {
    pub refresh_token: String,
    /// Free-form label stored with the session ("android", "web"...).
    #[serde(default)]
    pub client: String,
}

#[derive(Serialize)]
pub struct RefreshResponse {
    pub token: String,
    /// Only present on `/v1/auth/refresh-token`, where the token rotates.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_expires_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<UserView>,
}

#[derive(Serialize)]
pub struct ErrorBody {
    pub error: &'static str,
    pub message: String,
}

#[derive(Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    pub role: Role,
}

#[derive(Deserialize)]
pub struct UpdateUserRequest {
    pub role: Option<Role>,
    pub password: Option<String>,
}

#[derive(Deserialize)]
pub struct ChangePasswordRequest {
    pub old_password: String,
    pub new_password: String,
}

// ────────────────────────────────────────────────────────────────────
// Public auth routes
// ────────────────────────────────────────────────────────────────────

pub async fn login(
    Extension(state): Extension<AuthState>,
    Json(req): Json<LoginRequest>,
) -> Response {
    let user_opt = state.user_store.find_by_username(&req.username);
    let mut refresh_plain: Option<String> = None;
    let mut token_res: Option<String> = None;
    let mut view: Option<UserView> = None;
    match user_opt {
        Some(u) => {
            // EP-0012 P-004: Argon2 verify is CPU-bound (~100-500ms).
            // Running it on the tokio worker thread blocks other tasks.
            // Wrap in spawn_blocking so the worker stays free.
            let password = req.password.clone();
            let hash = u.password_hash.clone();
            let ok = tokio::task::spawn_blocking(move || {
                super::users::verify_password(&password, &hash).unwrap_or(false)
            })
            .await
            .unwrap_or(false);
            if !ok {
                // Wrong password: fall through with empty results so the
                // response is identical to an unknown user.
            } else {
                let _ = state.user_store.record_login(u.id);
                let t = issue_token(
                    &state.secret,
                    u.id,
                    &u.username,
                    u.role,
                    state.expiry_hours,
                )
                .ok();
                // Long-lived credential for the same login. Best-effort: if
                // the refresh store can't persist, the caller still gets a
                // working access token and `refresh_token` is omitted.
                let rt = state
                    .refresh_store
                    .issue(u.id, state.refresh_ttl_days, "login")
                    .ok()
                    .map(|(plain, _)| plain);
                refresh_plain = rt;
                token_res = t;
                view = Some(u.view());
            }
        }
        None => {
            // Verify against a dummy hash anyway so a missing user and a
            // wrong password take the same time (no user enumeration).
            let _ = super::users::verify_password(
                &req.password,
                "$argon2id$v=19$m=19456,t=2,p=1$AAAAAAAAAAAAAAAAAAAAAA$AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            );
        }
    }

    match (token_res, view) {
        (Some(token), Some(user)) => {
            let refresh = refresh_plain;
            // EP-0014 C-005: issue a fresh reauth_token (5min TTL).
            let reauth = state.reauth_tokens.issue();
            (
                StatusCode::OK,
                Json(LoginResponse {
                    token,
                    user,
                    reauth_token: reauth,
                    refresh_token: refresh,
                }),
            )
                .into_response()
        }
        _ => (
            StatusCode::UNAUTHORIZED,
            Json(ErrorBody {
                error: "unauthorized",
                message: "invalid credentials".into(),
            }),
        )
            .into_response(),
    }
}

/// Exchanges a long-lived refresh token for a fresh JWT, rotating it.
///
/// Unlike `/v1/auth/refresh` this needs no prior session, so the mobile can
/// boot straight into the panel. The presented token is invalidated as soon
/// as it is used.
pub async fn refresh_with_token(
    Extension(state): Extension<AuthState>,
    Json(req): Json<RefreshTokenRequest>,
) -> Response {
    let rotated = match state
        .refresh_store
        .validate_and_rotate(&req.refresh_token, state.refresh_ttl_days)
    {
        Ok(v) => v,
        Err(e) => {
            eprintln!("  ⚠️  refresh store error: {e}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody {
                    error: "internal",
                    message: "could not renew session".into(),
                }),
            )
                .into_response();
        }
    };

    let Some((grant, new_plain, expires_at)) = rotated else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorBody {
                error: "unauthorized",
                message: "refresh token invalid or expired".into(),
            }),
        )
            .into_response();
    };

    // The token survived rotation but the user may have been deleted since.
    let Some(user) = state.user_store.find_by_id(grant.user_id) else {
        let _ = state.refresh_store.revoke_all_for_user(grant.user_id);
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorBody {
                error: "unauthorized",
                message: "user no longer exists".into(),
            }),
        )
            .into_response();
    };

    match issue_token(
        &state.secret,
        user.id,
        &user.username,
        user.role,
        state.expiry_hours,
    ) {
        Ok(token) => (
            StatusCode::OK,
            Json(RefreshResponse {
                token,
                refresh_token: Some(new_plain),
                refresh_expires_at: Some(expires_at),
                user: Some(user.view()),
            }),
        )
            .into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorBody {
                error: "internal",
                message: "failed to issue token".into(),
            }),
        )
            .into_response(),
    }
}

/// Revokes a refresh token (logout from a device). Always answers 200 so a
/// client can't probe which tokens exist.
pub async fn revoke_refresh_token(
    Extension(state): Extension<AuthState>,
    Json(req): Json<RefreshTokenRequest>,
) -> Response {
    let _ = state.refresh_store.revoke(&req.refresh_token);
    (
        StatusCode::OK,
        Json(serde_json::json!({ "revoked": true })),
    )
        .into_response()
}

pub async fn refresh(
    Extension(state): Extension<AuthState>,
    Extension(user): Extension<UserContext>,
) -> Response {
    match issue_token(
        &state.secret,
        user.user_id,
        &user.username,
        user.role,
        state.expiry_hours,
    ) {
        Ok(token) => (
            StatusCode::OK,
            Json(RefreshResponse {
                token,
                refresh_token: None,
                refresh_expires_at: None,
                user: None,
            }),
        )
            .into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorBody {
                error: "internal",
                message: "failed to issue token".into(),
            }),
        )
            .into_response(),
    }
}

pub async fn logout() -> Response {
    StatusCode::NO_CONTENT.into_response()
}

// ────────────────────────────────────────────────────────────────────
// User CRUD (admin)
// ────────────────────────────────────────────────────────────────────

pub async fn list_users(
    Extension(state): Extension<AuthState>,
    _admin: RequireAdmin,
) -> Response {
    let users = state.user_store.list();
    (StatusCode::OK, Json(users)).into_response()
}

pub async fn create_user(
    Extension(state): Extension<AuthState>,
    _admin: RequireAdmin,
    Json(body): Json<CreateUserRequest>,
) -> Response {
    match state.user_store.create(&body.username, &body.password, body.role) {
        Ok(u) => (StatusCode::CREATED, Json(u)).into_response(),
        Err(e) => map_user_err(e),
    }
}

 pub async fn update_user(
    Extension(state): Extension<AuthState>,
    _admin: RequireAdmin,
    Path(id): Path<Uuid>,
    headers: axum::http::HeaderMap,
    Json(body): Json<UpdateUserRequest>,
) -> Response {
    // EP-0014 C-005: role changes require a fresh confirmation token
    // (TTL < 5min). Without it, any compromised Admin token could
    // silently escalate other users. Password changes don't require it.
    if body.role.is_some() {
        let reauth = headers
            .get("x-reauth-token")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if !state.reauth_tokens.validate(reauth) {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "error": "role change requires fresh X-Reauth-Token"
                })),
            )
                .into_response();
        }
    }
    if let Some(role) = body.role {
        if let Err(e) = state.user_store.update_role(id, role) {
            return map_user_err(e);
        }
    }
    if let Some(pwd) = body.password {
        if let Err(e) = state.user_store.admin_reset_password(id, &pwd) {
            return map_user_err(e);
        }
    }
    match state.user_store.find_by_id(id) {
        Some(u) => (StatusCode::OK, Json(u.view())).into_response(),
        None => not_found(),
    }
}

pub async fn delete_user(
    Extension(state): Extension<AuthState>,
    RequireAdmin(user): RequireAdmin,
    Path(id): Path<Uuid>,
) -> Response {
    if user.user_id == id {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorBody {
                error: "bad_request",
                message: "cannot delete the currently authenticated user".into(),
            }),
        )
            .into_response();
    }
    match state.user_store.delete(id) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => map_user_err(e),
    }
}

pub async fn change_my_password(
    Extension(state): Extension<AuthState>,
    Extension(user): Extension<UserContext>,
    Json(body): Json<ChangePasswordRequest>,
) -> Response {
    match state
        .user_store
        .update_password(user.user_id, &body.old_password, &body.new_password)
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(UserStoreError::InvalidPassword) => (
            StatusCode::UNAUTHORIZED,
            Json(ErrorBody {
                error: "unauthorized",
                message: "old password is incorrect".into(),
            }),
        )
            .into_response(),
        Err(e) => map_user_err(e),
    }
}

// ────────────────────────────────────────────────────────────────────
// Helpers
// ────────────────────────────────────────────────────────────────────

fn map_user_err(e: UserStoreError) -> Response {
    let (code, label, msg) = match &e {
        UserStoreError::NotFound => (StatusCode::NOT_FOUND, "not_found", e.to_string()),
        UserStoreError::UsernameTaken => {
            (StatusCode::CONFLICT, "conflict", e.to_string())
        }
        UserStoreError::InvalidPassword => (
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            e.to_string(),
        ),
        UserStoreError::InvalidUsername | UserStoreError::PasswordTooShort => {
            (StatusCode::BAD_REQUEST, "bad_request", e.to_string())
        }
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal",
            e.to_string(),
        ),
    };
    (code, Json(ErrorBody { error: label, message: msg })).into_response()
}

fn not_found() -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(ErrorBody {
            error: "not_found",
            message: "user not found".into(),
        }),
    )
        .into_response()
}

// ────────────────────────────────────────────────────────────────────
// Routers
// ────────────────────────────────────────────────────────────────────

/// Public auth routes (mounted at `/v1/auth/*`).
pub fn auth_routes() -> Router {
    Router::new()
        .route("/v1/auth/login", post(login))
        .route("/v1/auth/refresh", post(refresh))
        .route("/v1/auth/logout", post(logout))
        // Cold-start session renewal: takes the long-lived refresh token
        // instead of a JWT, so the mobile can get a token with no session
        // at all. Rotates the refresh token on every call.
        .route("/v1/auth/refresh-token", post(refresh_with_token))
        .route("/v1/auth/revoke-token", post(revoke_refresh_token))
        // Google Sign-In. 404 while `auth.google_client_id` is empty.
        .route("/v1/auth/google", post(super::google::google_sign_in))
}

/// Admin user CRUD (mounted at `/v1/users*`).
pub fn users_routes() -> Router {
    Router::new()
        .route(
            "/v1/users",
            get(list_users).post(create_user),
        )
        .route(
            "/v1/users/:id",
            patch(update_user).delete(delete_user),
        )
        .route("/v1/users/me/password", patch(change_my_password))
}