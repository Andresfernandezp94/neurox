// Refresh tokens: credenciales de larga vida para renovar el JWT de acceso.
//
// Por qué existe: el JWT de acceso caduca a `auth.jwt_expiry_hours` (24 h por
// defecto) y eso lo mantiene corto a propósito. Un cliente que solo pueda
// guardar el access token tiene que volver a escribir la contraseña cada día,
// que es exactamente lo que un móvil no debería hacer.
//
// El refresh token no es un JWT: es un valor opaco aleatorio que solo se
// muestra una vez, en el `issue()`. En disco queda su SHA-256, así que un
// volcado del fichero no permite autenticarse. Cada uso rota el token y
// revoca el anterior, de modo que un token robado deja de servir en cuanto
// el dueño legítimo lo usa una vez.
//
// A diferencia de `ReauthTokens` (que vive en memoria y dura 5 min para
// confirmar cambios de rol), esto se persiste y dura días.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use uuid::Uuid;

#[derive(Debug)]
pub enum RefreshStoreError {
    Io(std::io::Error),
    Serde(serde_json::Error),
}

impl std::fmt::Display for RefreshStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "refresh store io: {e}"),
            Self::Serde(e) => write!(f, "refresh store json: {e}"),
        }
    }
}

impl std::error::Error for RefreshStoreError {}

impl From<std::io::Error> for RefreshStoreError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<serde_json::Error> for RefreshStoreError {
    fn from(e: serde_json::Error) -> Self {
        Self::Serde(e)
    }
}

/// Lo que se persiste: nunca el token en claro, solo su hash.
#[derive(Debug, Serialize, Deserialize, Clone)]
struct StoredRefresh {
    id: Uuid,
    user_id: Uuid,
    /// SHA-256 en hex del token. Es la única representación en disco.
    token_hash: String,
    issued_at: i64,
    expires_at: i64,
    /// Etiqueta libre para saber desde dónde se emitió ("web", "android"...).
    #[serde(default)]
    client: String,
}

/// Lo que devuelve `validate`: lo mínimo para emitir un JWT nuevo.
#[derive(Debug, Clone)]
pub struct RefreshGrant {
    pub user_id: Uuid,
}

/// JSON-backed refresh token store. Thread-safe via `RwLock`.
pub struct RefreshStore {
    path: PathBuf,
    inner: RwLock<HashMap<Uuid, StoredRefresh>>,
}

/// 32 bytes de entropía en hex = 64 caracteres.
fn random_token() -> String {
    use rand::RngCore;
    let mut buf = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut buf);
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

fn hash_token(token: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(token.as_bytes());
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

impl RefreshStore {
    /// Carga desde disco. Si el fichero no existe, store vacío (no se crea
    /// hasta el primer write).
    pub fn load(path: &Path) -> Result<Self, RefreshStoreError> {
        if !path.exists() {
            return Ok(Self {
                path: path.to_path_buf(),
                inner: RwLock::new(HashMap::new()),
            });
        }
        let text = std::fs::read_to_string(path)?;
        let list: Vec<StoredRefresh> = serde_json::from_str(&text)?;
        let map = list.into_iter().map(|r| (r.id, r)).collect();
        Ok(Self {
            path: path.to_path_buf(),
            inner: RwLock::new(map),
        })
    }

    /// Store solo en memoria, sin ruta: nada se persiste. Para los tests y
    /// para cuando el store de disco no se pudo cargar (sin él el daemon
    /// arranca igual, solo que los refresh tokens no sobreviven al reinicio).
    pub fn in_memory() -> Self {
        Self {
            path: PathBuf::new(),
            inner: RwLock::new(HashMap::new()),
        }
    }

    /// Atomic write: tempfile, fsync, rename, y `0600` porque contiene
    /// hashes que permiten renovar sesiones.
    fn persist(&self, map: &HashMap<Uuid, StoredRefresh>) -> Result<(), RefreshStoreError> {
        use std::io::Write;
        // Ruta vacía ⇒ store en memoria: no hay dónde escribir.
        if self.path.as_os_str().is_empty() {
            return Ok(());
        }
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        let mut f = std::fs::File::create(&tmp)?;
        let json = serde_json::to_vec_pretty(&map.values().collect::<Vec<_>>())?;
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

    /// Emite un token nuevo para `user_id`. Devuelve el valor en claro: es
    /// la única vez que existe fuera de la memoria del cliente.
    pub fn issue(
        &self,
        user_id: Uuid,
        ttl_days: u64,
        client: &str,
    ) -> Result<(String, i64), RefreshStoreError> {
        let now = chrono::Utc::now().timestamp();
        let expires_at = now + (ttl_days as i64) * 86_400;
        let plain = random_token();
        let entry = StoredRefresh {
            id: Uuid::new_v4(),
            user_id,
            token_hash: hash_token(&plain),
            issued_at: now,
            expires_at,
            client: client.to_string(),
        };
        let mut map = self.inner.write().map_err(|_| {
            RefreshStoreError::Io(std::io::Error::other("refresh store lock poisoned"))
        })?;
        map.insert(entry.id, entry);
        self.persist(&map)?;
        Ok((plain, expires_at))
    }

    /// Valida un token y lo rota en el mismo paso: el antiguo queda revocado.
    /// Si no hay coincidencia, está caducado o expiró, devuelve `None`.
    pub fn validate_and_rotate(
        &self,
        plain: &str,
        ttl_days: u64,
    ) -> Result<Option<(RefreshGrant, String, i64)>, RefreshStoreError> {
        let hash = hash_token(plain);
        let now = chrono::Utc::now().timestamp();

        let mut map = self
            .inner
            .write()
            .map_err(|_| RefreshStoreError::Io(std::io::Error::other("lock poisoned")))?;

        let found = map
            .values()
            .find(|r| r.token_hash == hash)
            .map(|r| (r.id, r.user_id, r.expires_at));

        let Some((id, user_id, expires_at)) = found else {
            return Ok(None);
        };
        // `expires_at` es exclusivo: un token que caduca "ahora" ya no vale.
        if expires_at <= now {
            map.remove(&id);
            let _ = self.persist(&map);
            return Ok(None);
        }

        // Rota: el token usado ya no sirve.
        map.remove(&id);
        let new_plain = random_token();
        let new_expires = now + (ttl_days as i64) * 86_400;
        // La clave del mapa y StoredRefresh::id deben ser el mismo UUID: el
        // resto del store (revoke, revoke_all_for_user, purge_expired)
        // busca por `r.id` y no borraría nada si difieren.
        let new_id = Uuid::new_v4();
        map.insert(
            new_id,
            StoredRefresh {
                id: new_id,
                user_id,
                token_hash: hash_token(&new_plain),
                issued_at: now,
                expires_at: new_expires,
                client: String::new(),
            },
        );
        self.persist(&map)?;
        Ok(Some((
            RefreshGrant { user_id },
            new_plain,
            new_expires,
        )))
    }

    /// Revoca un token concreto. Lo usa el logout para que un token
    /// capturado no sobreviva a la sesión.
    pub fn revoke(&self, plain: &str) -> Result<bool, RefreshStoreError> {
        let hash = hash_token(plain);
        let mut map = self
            .inner
            .write()
            .map_err(|_| RefreshStoreError::Io(std::io::Error::other("lock poisoned")))?;
        let hit = map.values().find(|r| r.token_hash == hash).map(|r| r.id);
        match hit {
            Some(id) => {
                map.remove(&id);
                self.persist(&map)?;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// Revoca todos los tokens de un usuario. Para "cerrar sesión en todos
    /// los dispositivos" y para rotar credenciales.
    pub fn revoke_all_for_user(&self, user_id: Uuid) -> Result<usize, RefreshStoreError> {
        let mut map = self
            .inner
            .write()
            .map_err(|_| RefreshStoreError::Io(std::io::Error::other("lock poisoned")))?;
        let ids: Vec<Uuid> = map
            .values()
            .filter(|r| r.user_id == user_id)
            .map(|r| r.id)
            .collect();
        for id in &ids {
            map.remove(id);
        }
        let n = ids.len();
        if n > 0 {
            self.persist(&map)?;
        }
        Ok(n)
    }

    /// Purga los caducados. Lo llama el login para que el fichero no crezca
    /// sin límite.
    pub fn purge_expired(&self) -> Result<usize, RefreshStoreError> {
        let now = chrono::Utc::now().timestamp();
        let mut map = self
            .inner
            .write()
            .map_err(|_| RefreshStoreError::Io(std::io::Error::other("lock poisoned")))?;
        let ids: Vec<Uuid> = map
            .values()
            .filter(|r| r.expires_at <= now)
            .map(|r| r.id)
            .collect();
        for id in &ids {
            map.remove(id);
        }
        let n = ids.len();
        if n > 0 {
            self.persist(&map)?;
        }
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp() -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("neurox-refresh-{}.json", Uuid::new_v4()));
        p
    }

    #[test]
    fn issue_then_validate_rotates() {
        let path = tmp();
        let store = RefreshStore::load(&path).unwrap();
        let uid = Uuid::new_v4();
        let (plain, _) = store.issue(uid, 30, "android").unwrap();

        let (grant, next, _) = store
            .validate_and_rotate(&plain, 30)
            .unwrap()
            .expect("debería validar");
        assert_eq!(grant.user_id, uid);

        // El token antiguo ya no vale.
        assert!(store.validate_and_rotate(&plain, 30).unwrap().is_none());
        // El nuevo sí.
        assert!(store
            .validate_and_rotate(&next, 30)
            .unwrap()
            .is_some());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn unknown_token_is_rejected() {
        let path = tmp();
        let store = RefreshStore::load(&path).unwrap();
        store.issue(Uuid::new_v4(), 30, "web").unwrap();
        assert!(store.validate_and_rotate("nada", 30).unwrap().is_none());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn expired_token_is_rejected_and_dropped() {
        let path = tmp();
        let store = RefreshStore::load(&path).unwrap();
        let uid = Uuid::new_v4();
        let (plain, _) = store.issue(uid, 0, "web").unwrap();
        // ttl 0 => expira en el mismo segundo.
        assert!(store.validate_and_rotate(&plain, 30).unwrap().is_none());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn revoke_and_revoke_all() {
        let path = tmp();
        let store = RefreshStore::load(&path).unwrap();
        let uid = Uuid::new_v4();
        let (a, _) = store.issue(uid, 30, "web").unwrap();
        let (_b, _) = store.issue(uid, 30, "android").unwrap();

        assert!(store.revoke(&a).unwrap());
        assert!(!store.revoke(&a).unwrap());
        assert_eq!(store.revoke_all_for_user(uid).unwrap(), 1);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn file_is_owner_only() {
        let path = tmp();
        let store = RefreshStore::load(&path).unwrap();
        store.issue(Uuid::new_v4(), 30, "web").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600, "los hashes quedan solo para el dueño");
        }
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn plain_token_is_never_persisted() {
        let path = tmp();
        let store = RefreshStore::load(&path).unwrap();
        let (plain, _) = store.issue(Uuid::new_v4(), 30, "android").unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains(&plain), "el token en claro no puede estar en disco");
        let _ = std::fs::remove_file(path);
    }
}