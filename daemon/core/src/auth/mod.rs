// EP-0007: Authentication & authorization.
//
// Public API:
//   - [`users`]: `User` struct + JSON-backed `UserStore` (CRUD).
//   - [`tokens`]: `issue_token` / `verify_token` (HS256 JWT).
//   - [`middleware`]: `JwtAuthLayer` + `Role` enum + `UserContext` extractor.
//   - [`handlers`]: HTTP handlers for /v1/auth/* and /v1/users/*.
//
// The auth module is feature-gated behind `auth.enabled: true` in
// `CoreConfig`. When disabled, no middleware runs and the legacy
// `api_token` behavior applies.

pub mod cognito;
pub mod google;
pub mod handlers;
#[cfg(test)]
mod handlers_tests;
pub mod middleware;
pub mod refresh;
pub mod tokens;
pub mod users;

pub use handlers::{auth_routes, users_routes, AuthState, ReauthTokens};
pub use middleware::{JwtAuthLayer, Role, UserContext};
pub use refresh::{RefreshGrant, RefreshStore, RefreshStoreError};
pub use tokens::{issue_token, verify_token, JwtError, JwtSecret};
pub use users::{User, UserStore, UserStoreError};

use crate::config::CoreConfig;
use std::path::PathBuf;

/// Resolved auth config with absolute paths (callers don't need to know
/// about `~` expansion semantics).
#[derive(Debug, Clone)]
pub struct AuthConfig {
    pub enabled: bool,
    pub user_store_path: PathBuf,
    pub jwt_secret_path: PathBuf,
    pub jwt_expiry_hours: u64,
    pub refresh_store_path: PathBuf,
    pub refresh_ttl_days: u64,
    pub google_client_id: String,
    pub google_client_secret: String,
    pub google_audience: String,
    pub cognito_region: String,
    pub cognito_user_pool_id: String,
    pub cognito_client_id: String,
}

impl Default for AuthConfig {
    fn default() -> Self {
        // Same defaults as AuthConfigSection. Used by CLI commands
        // (e.g. `users reset-password`) that don't load the full config.
        use crate::config::{
            default_jwt_expiry_hours, default_jwt_secret_path, default_user_store_path,
        };
        Self {
            enabled: false,
            user_store_path: default_user_store_path(),
            jwt_secret_path: default_jwt_secret_path(),
            jwt_expiry_hours: default_jwt_expiry_hours(),
            refresh_store_path: crate::config::default_refresh_store_path(),
            refresh_ttl_days: crate::config::default_refresh_ttl_days(),
            google_client_id: String::new(),
            google_client_secret: String::new(),
            google_audience: String::new(),
            cognito_region: String::new(),
            cognito_user_pool_id: String::new(),
            cognito_client_id: String::new(),
        }
    }
}

impl AuthConfig {
    pub fn from_core(core: &CoreConfig) -> Self {
        Self {
            enabled: core.auth.enabled,
            user_store_path: core.auth.user_store_path.clone(),
            jwt_secret_path: core.auth.jwt_secret_path.clone(),
            jwt_expiry_hours: core.auth.jwt_expiry_hours,
            refresh_store_path: core.auth.refresh_store_path.clone(),
            refresh_ttl_days: core.auth.refresh_ttl_days,
            google_client_id: core.auth.google_client_id.clone(),
            google_client_secret: core.auth.google_client_secret.clone(),
            google_audience: core.auth.google_audience.clone(),
            cognito_region: core.auth.cognito_region.clone(),
            cognito_user_pool_id: core.auth.cognito_user_pool_id.clone(),
            cognito_client_id: core.auth.cognito_client_id.clone(),
        }
    }
}