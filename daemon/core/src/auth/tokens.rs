// EP-0007 — JWT issue/verify.
//
// Algorithm: HS256. Secret is loaded from disk (auto-generated on first
// run if missing). Claims are JSON-encoded with the standard `sub`,
// `username`, `role`, `iat`, `exp` fields.
//
// The secret is zeroized on drop to avoid leaving it in memory.

use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::path::Path;
use uuid::Uuid;
use zeroize::Zeroize;

use super::users::Role;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    /// User UUID as string.
    pub sub: String,
    pub username: String,
    pub role: Role,
    /// Issued at (unix seconds).
    pub iat: i64,
    /// Expiry (unix seconds).
    pub exp: i64,
}

#[derive(Debug)]
pub enum JwtError {
    Io(std::io::Error),
    Json(serde_json::Error),
    Jwt(jsonwebtoken::errors::Error),
    SecretMissing,
}

impl fmt::Display for JwtError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JwtError::Io(e) => write!(f, "io error: {e}"),
            JwtError::Json(e) => write!(f, "json error: {e}"),
            JwtError::Jwt(e) => write!(f, "jwt error: {e}"),
            JwtError::SecretMissing => f.write_str("jwt secret missing"),
        }
    }
}

impl std::error::Error for JwtError {}

impl From<std::io::Error> for JwtError {
    fn from(e: std::io::Error) -> Self {
        JwtError::Io(e)
    }
}
impl From<serde_json::Error> for JwtError {
    fn from(e: serde_json::Error) -> Self {
        JwtError::Json(e)
    }
}
impl From<jsonwebtoken::errors::Error> for JwtError {
    fn from(e: jsonwebtoken::errors::Error) -> Self {
        JwtError::Jwt(e)
    }
}

/// Wraps a 64-byte HMAC secret. Zeroed on drop.
pub struct JwtSecret {
    bytes: Vec<u8>,
}

impl Zeroize for JwtSecret {
    fn zeroize(&mut self) {
        self.bytes.zeroize();
    }
}

impl Drop for JwtSecret {
    fn drop(&mut self) {
        self.zeroize();
    }
}

impl JwtSecret {
    /// Generate a fresh 64-byte secret.
    pub fn generate() -> Self {
        use rand::RngCore;
        let mut bytes = vec![0u8; 64];
        rand::thread_rng().fill_bytes(&mut bytes);
        Self { bytes }
    }

    /// Load from a JSON file `{ "secret": "<base64>" }`, or generate + persist
    /// if the file does not exist.
    pub fn load_or_create(path: &Path) -> Result<Self, JwtError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        if path.exists() {
            let text = fs::read_to_string(path)?;
            let stored: StoredSecret = serde_json::from_str(&text)?;
            Ok(Self {
                bytes: stored.decode()?,
            })
        } else {
            let s = Self::generate();
            let encoded = StoredSecret::from_bytes(&s.bytes);
            let json = serde_json::to_vec_pretty(&encoded)?;
            fs::write(path, json)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(path)?.permissions();
                perms.set_mode(0o600);
                fs::set_permissions(path, perms)?;
            }
            Ok(s)
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

#[derive(Serialize, Deserialize)]
struct StoredSecret {
    /// Base64 (standard, with padding).
    secret: String,
}

impl StoredSecret {
    fn from_bytes(b: &[u8]) -> Self {
        use base64::Engine;
        Self {
            secret: base64::engine::general_purpose::STANDARD.encode(b),
        }
    }
    fn decode(&self) -> Result<Vec<u8>, JwtError> {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD
            .decode(&self.secret)
            .map_err(|e| JwtError::Json(serde_json::Error::io(
                std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()),
            )))
    }
}

/// Convenience: load secret + encode a JWT for a user.
pub fn issue_token(
    secret: &JwtSecret,
    user_id: Uuid,
    username: &str,
    role: Role,
    expiry_hours: u64,
) -> Result<String, JwtError> {
    let now = Utc::now();
    let exp = now + Duration::hours(expiry_hours as i64);
    let claims = Claims {
        sub: user_id.to_string(),
        username: username.to_string(),
        role,
        iat: now.timestamp(),
        exp: exp.timestamp(),
    };
    let header = Header::new(jsonwebtoken::Algorithm::HS256);
    let key = EncodingKey::from_secret(secret.as_bytes());
    encode(&header, &claims, &key).map_err(JwtError::from)
}

/// Decode + validate a JWT. Returns the claims if valid.
pub fn verify_token(secret: &JwtSecret, token: &str) -> Result<Claims, JwtError> {
    let key = DecodingKey::from_secret(secret.as_bytes());
    let mut validation = Validation::new(jsonwebtoken::Algorithm::HS256);
    validation.leeway = 5; // small clock-skew tolerance
    let data = decode::<Claims>(token, &key, &validation)?;
    Ok(data.claims)
}

/// Extract bearer token from `Authorization` header value.
pub fn extract_bearer(header: Option<&axum::http::HeaderValue>) -> Option<&str> {
    header
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn issue_and_verify_roundtrip() {
        let secret = JwtSecret::generate();
        let user_id = Uuid::new_v4();
        let token = issue_token(&secret, user_id, "tester", Role::Admin, 1).unwrap();
        let claims = verify_token(&secret, &token).unwrap();
        assert_eq!(claims.username, "tester");
        assert_eq!(claims.role, Role::Admin);
        assert_eq!(claims.sub, user_id.to_string());
    }

    #[test]
    fn verify_rejects_wrong_secret() {
        let s1 = JwtSecret::generate();
        let s2 = JwtSecret::generate();
        let token = issue_token(&s1, Uuid::new_v4(), "u", Role::Admin, 1).unwrap();
        let err = verify_token(&s2, &token).unwrap_err();
        assert!(matches!(err, JwtError::Jwt(_)));
    }

    #[test]
    fn verify_rejects_expired() {
        let secret = JwtSecret::generate();
        let now = Utc::now();
        let claims = Claims {
            sub: Uuid::new_v4().to_string(),
            username: "u".into(),
            role: Role::Admin,
            iat: (now - Duration::hours(2)).timestamp(),
            exp: (now - Duration::hours(1)).timestamp(),
        };
        let token = encode(
            &Header::new(jsonwebtoken::Algorithm::HS256),
            &claims,
            &EncodingKey::from_secret(secret.as_bytes()),
        )
        .unwrap();
        let err = verify_token(&secret, &token).unwrap_err();
        assert!(matches!(err, JwtError::Jwt(_)));
    }

    #[test]
    fn load_or_create_persists() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("jwt_secret");
        let s1 = JwtSecret::load_or_create(&path).unwrap();
        assert!(path.exists());
        // load again, should be the same secret
        let s2 = JwtSecret::load_or_create(&path).unwrap();
        assert_eq!(s1.as_bytes(), s2.as_bytes());
    }

    #[test]
    fn extract_bearer_parses_header() {
        let h = axum::http::HeaderValue::from_static("Bearer abc.def.ghi");
        assert_eq!(extract_bearer(Some(&h)), Some("abc.def.ghi"));
        let h2 = axum::http::HeaderValue::from_static("Basic xyz");
        assert_eq!(extract_bearer(Some(&h2)), None);
    }
}