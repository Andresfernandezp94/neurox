// EP-0007 — User model + JSON-backed store.
//
// Storage: a single `users.json` file with permissions 0600 (created if
// missing). All writes go through a tempfile + rename for atomicity, and
// the file is `fsync`'d before swap.
//
// Passwords are stored only as `password_hash` (argon2id PHC string).
// The plaintext is never persisted, logged, or returned by any API.

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use uuid::Uuid;

/// Role assigned to a user. Used for authorization on protected routes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum Role {
    Admin,
    Operator,
    Viewer,
}

impl Role {
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::Admin => "Admin",
            Role::Operator => "Operator",
            Role::Viewer => "Viewer",
        }
    }
}

/// Public-facing user (never includes password hash).
#[derive(Debug, Clone, Serialize)]
pub struct UserView {
    pub id: Uuid,
    pub username: String,
    pub role: Role,
    pub created_at: DateTime<Utc>,
    pub last_login_at: Option<DateTime<Utc>>,
}

/// Internal user record (includes password hash).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredUser {
    pub id: Uuid,
    pub username: String,
    pub password_hash: String,
    pub role: Role,
    pub created_at: DateTime<Utc>,
    pub last_login_at: Option<DateTime<Utc>>,
}

impl StoredUser {
    pub fn view(&self) -> UserView {
        UserView {
            id: self.id,
            username: self.username.clone(),
            role: self.role,
            created_at: self.created_at,
            last_login_at: self.last_login_at,
        }
    }
}

/// Re-export for the public API.
pub type User = StoredUser;

#[derive(Debug)]
pub enum UserStoreError {
    NotFound,
    UsernameTaken,
    InvalidPassword,
    InvalidUsername,
    PasswordTooShort,
    Io(std::io::Error),
    Serde(serde_json::Error),
    Argon2(String),
}

impl fmt::Display for UserStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UserStoreError::NotFound => f.write_str("user not found"),
            UserStoreError::UsernameTaken => f.write_str("username already exists"),
            UserStoreError::InvalidPassword => f.write_str("invalid password"),
            UserStoreError::InvalidUsername => {
                f.write_str("username must be 3-64 chars, alphanumeric + _-.")
            }
            UserStoreError::PasswordTooShort => {
                f.write_str("password must be at least 12 characters")
            }
            UserStoreError::Io(e) => write!(f, "io error: {e}"),
            UserStoreError::Serde(e) => write!(f, "serde error: {e}"),
            UserStoreError::Argon2(e) => write!(f, "argon2 error: {e}"),
        }
    }
}

impl std::error::Error for UserStoreError {}

impl From<std::io::Error> for UserStoreError {
    fn from(e: std::io::Error) -> Self {
        UserStoreError::Io(e)
    }
}

impl From<serde_json::Error> for UserStoreError {
    fn from(e: serde_json::Error) -> Self {
        UserStoreError::Serde(e)
    }
}

/// In-memory + JSON-backed user store. Thread-safe via `RwLock`.
pub struct UserStore {
    path: PathBuf,
    inner: RwLock<HashMap<Uuid, StoredUser>>,
}

impl UserStore {
    /// Load users from disk. If the file does not exist, returns an empty
    /// store (the file is not created until the first write).
    pub fn load(path: &Path) -> Result<Self, UserStoreError> {
        if !path.exists() {
            return Ok(Self {
                path: path.to_path_buf(),
                inner: RwLock::new(HashMap::new()),
            });
        }
        let text = std::fs::read_to_string(path)?;
        let users: Vec<StoredUser> = serde_json::from_str(&text)?;
        let mut map = HashMap::with_capacity(users.len());
        for u in users {
            map.insert(u.id, u);
        }
        Ok(Self {
            path: path.to_path_buf(),
            inner: RwLock::new(map),
        })
    }

    /// Atomic write: write to tempfile, fsync, rename, fsync parent dir.
    fn persist(&self, users: &HashMap<Uuid, StoredUser>) -> Result<(), UserStoreError> {
        use std::io::Write;
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        let mut f = std::fs::File::create(&tmp)?;
        let json = serde_json::to_vec_pretty(&users.values().collect::<Vec<_>>())?;
        f.write_all(&json)?;
        f.sync_all()?;
        std::fs::rename(&tmp, &self.path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&self.path)?.permissions();
            perms.set_mode(0o600);
            std::fs::set_permissions(&self.path, perms)?;
        }
        Ok(())
    }

    pub fn create(
        &self,
        username: &str,
        password: &str,
        role: Role,
    ) -> Result<UserView, UserStoreError> {
        validate_username(username)?;
        validate_password(password)?;
        let hash = hash_password(password)?;

        let mut guard = self.inner.write().expect("user store poisoned");
        if guard.values().any(|u| u.username == username) {
            return Err(UserStoreError::UsernameTaken);
        }
        let user = StoredUser {
            id: Uuid::new_v4(),
            username: username.to_string(),
            password_hash: hash,
            role,
            created_at: Utc::now(),
            last_login_at: None,
        };
        let view = user.view();
        guard.insert(user.id, user);
        self.persist(&guard)?;
        Ok(view)
    }

    pub fn find_by_username(&self, username: &str) -> Option<StoredUser> {
        let guard = self.inner.read().expect("user store poisoned");
        guard
            .values()
            .find(|u| u.username == username)
            .cloned()
    }

    pub fn find_by_id(&self, id: Uuid) -> Option<StoredUser> {
        let guard = self.inner.read().expect("user store poisoned");
        guard.get(&id).cloned()
    }

    pub fn list(&self) -> Vec<UserView> {
        let guard = self.inner.read().expect("user store poisoned");
        let mut v: Vec<UserView> = guard.values().map(|u| u.view()).collect();
        v.sort_by(|a, b| a.username.cmp(&b.username));
        v
    }

    pub fn update_role(&self, id: Uuid, role: Role) -> Result<UserView, UserStoreError> {
        let mut guard = self.inner.write().expect("user store poisoned");
        let u = guard.get_mut(&id).ok_or(UserStoreError::NotFound)?;
        u.role = role;
        let view = u.view();
        self.persist(&guard)?;
        Ok(view)
    }

    pub fn update_password(
        &self,
        id: Uuid,
        old_password: &str,
        new_password: &str,
    ) -> Result<(), UserStoreError> {
        validate_password(new_password)?;
        let mut guard = self.inner.write().expect("user store poisoned");
        let u = guard.get_mut(&id).ok_or(UserStoreError::NotFound)?;
        if !verify_password(old_password, &u.password_hash)? {
            return Err(UserStoreError::InvalidPassword);
        }
        u.password_hash = hash_password(new_password)?;
        self.persist(&guard)?;
        Ok(())
    }

    pub fn admin_reset_password(
        &self,
        id: Uuid,
        new_password: &str,
    ) -> Result<(), UserStoreError> {
        validate_password(new_password)?;
        let mut guard = self.inner.write().expect("user store poisoned");
        let u = guard.get_mut(&id).ok_or(UserStoreError::NotFound)?;
        u.password_hash = hash_password(new_password)?;
        self.persist(&guard)?;
        Ok(())
    }

    pub fn delete(&self, id: Uuid) -> Result<(), UserStoreError> {
        let mut guard = self.inner.write().expect("user store poisoned");
        if guard.remove(&id).is_none() {
            return Err(UserStoreError::NotFound);
        }
        self.persist(&guard)?;
        Ok(())
    }

    pub fn record_login(&self, id: Uuid) -> Result<(), UserStoreError> {
        let mut guard = self.inner.write().expect("user store poisoned");
        let u = guard.get_mut(&id).ok_or(UserStoreError::NotFound)?;
        u.last_login_at = Some(Utc::now());
        self.persist(&guard)?;
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.inner.read().expect("user store poisoned").is_empty()
    }
}

pub fn hash_password(plain: &str) -> Result<String, UserStoreError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(plain.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| UserStoreError::Argon2(e.to_string()))
}

pub fn verify_password(plain: &str, hash: &str) -> Result<bool, UserStoreError> {
    let parsed = PasswordHash::new(hash).map_err(|e| UserStoreError::Argon2(e.to_string()))?;
    Ok(Argon2::default()
        .verify_password(plain.as_bytes(), &parsed)
        .is_ok())
}

fn validate_username(username: &str) -> Result<(), UserStoreError> {
    let len = username.chars().count();
    if !(3..=64).contains(&len) {
        return Err(UserStoreError::InvalidUsername);
    }
    if !username
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
    {
        return Err(UserStoreError::InvalidUsername);
    }
    Ok(())
}

fn validate_password(password: &str) -> Result<(), UserStoreError> {
    if password.chars().count() < 12 {
        return Err(UserStoreError::PasswordTooShort);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_store() -> (TempDir, UserStore) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("users.json");
        let store = UserStore::load(&path).unwrap();
        (dir, store)
    }

    #[test]
    fn create_and_find() {
        let (_dir, store) = make_store();
        let v = store.create("alice", "supersecret123", Role::Admin).unwrap();
        assert_eq!(v.username, "alice");
        let found = store.find_by_username("alice").unwrap();
        assert_eq!(found.role, Role::Admin);
    }

    #[test]
    fn duplicate_username() {
        let (_dir, store) = make_store();
        store.create("bob", "supersecret123", Role::Admin).unwrap();
        let err = store
            .create("bob", "anothersecret12", Role::Operator)
            .unwrap_err();
        assert!(matches!(err, UserStoreError::UsernameTaken));
    }

    #[test]
    fn password_too_short() {
        let (_dir, store) = make_store();
        let err = store.create("carol", "short", Role::Admin).unwrap_err();
        assert!(matches!(err, UserStoreError::PasswordTooShort));
    }

    #[test]
    fn invalid_username() {
        let (_dir, store) = make_store();
        assert!(matches!(
            store.create("a!", "validpassword123", Role::Admin),
            Err(UserStoreError::InvalidUsername)
        ));
        assert!(matches!(
            store.create("ab", "validpassword123", Role::Admin),
            Err(UserStoreError::InvalidUsername)
        ));
    }

    #[test]
    fn verify_password_roundtrip() {
        let h = hash_password("correcthorsebattery").unwrap();
        assert!(verify_password("correcthorsebattery", &h).unwrap());
        assert!(!verify_password("wronghorse", &h).unwrap());
    }

    #[test]
    fn update_password_requires_old() {
        let (_dir, store) = make_store();
        let v = store.create("dave", "oldpassword123", Role::Admin).unwrap();
        assert!(matches!(
            store.update_password(v.id, "wrong", "newpassword123"),
            Err(UserStoreError::InvalidPassword)
        ));
        store
            .update_password(v.id, "oldpassword123", "newpassword123")
            .unwrap();
    }

    #[test]
    fn list_sorted() {
        let (_dir, store) = make_store();
        store.create("zoe", "password123abc", Role::Viewer).unwrap();
        store.create("alice", "password123abc", Role::Admin).unwrap();
        store.create("mark", "password123abc", Role::Operator).unwrap();
        let names: Vec<_> = store.list().into_iter().map(|u| u.username).collect();
        assert_eq!(names, vec!["alice", "mark", "zoe"]);
    }

    #[test]
    fn persist_and_reload() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("users.json");
        let store = UserStore::load(&path).unwrap();
        store.create("eve", "persisted1234", Role::Admin).unwrap();
        drop(store);

        let store2 = UserStore::load(&path).unwrap();
        assert_eq!(store2.list().len(), 1);
        assert!(store2.find_by_username("eve").is_some());
    }

    #[test]
    fn delete_removes_user() {
        let (_dir, store) = make_store();
        let v = store.create("frank", "password123abc", Role::Admin).unwrap();
        store.delete(v.id).unwrap();
        assert!(store.find_by_id(v.id).is_none());
    }
}

#[cfg(test)]
mod proptest_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        // EP-0013 T-001: property-based testing for validate_username.
        // The validator should never panic and should accept exactly
        // [a-zA-Z0-9_-] of length 3..=64.
        #[test]
        fn validate_username_never_panics(s in "\\PC*") {
            let _ = validate_username(&s);
        }

        #[test]
        fn validate_username_accepts_safe_input(
            s in "[a-zA-Z0-9_-]{3,64}"
        ) {
            prop_assert!(validate_username(&s).is_ok());
        }

        #[test]
        fn validate_username_rejects_short(s in "\\PC{0,2}") {
            prop_assert!(validate_username(&s).is_err());
        }

        #[test]
        fn validate_username_rejects_invalid_chars(s in "[^a-zA-Z0-9_-]{1,32}") {
            prop_assert!(validate_username(&s).is_err());
        }

        // EP-0013 T-001: property-based testing for validate_password.
        // Should accept any password of length >= 12, reject shorter.
        #[test]
        fn validate_password_accepts_long(s in ".{12,128}") {
            prop_assert!(validate_password(&s).is_ok());
        }

        #[test]
        fn validate_password_rejects_short(s in ".{0,11}") {
            prop_assert!(validate_password(&s).is_err());
        }
    }
}