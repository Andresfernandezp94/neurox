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

pub mod handlers;
pub mod middleware;
pub mod tokens;
pub mod users;

pub use handlers::{auth_routes, users_routes, AuthState, ReauthTokens};
pub use middleware::{JwtAuthLayer, Role, UserContext};
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
        }
    }
}