// Preferencias de LLM por usuario.
//
// El default de provider/modelo es config DEL USUARIO, no del daemon.
// Con un unico admin la diferencia no se nota, pero con dos usuarios el
// store global `engine_settings` los pisa: si A elige minimax y B elige
// openrouter, el ultimo en escribir le gana al otro. Por eso esta tabla
// existe aparte y su clave primaria es `user_id`.
//
// Precedencia de resolucion (de mayor a menor), implementada en
// `resolve_llm_pref`:
//
//   1. esta tabla          — lo que seteo el usuario en el front
//   2. NEUROX_DEFAULT_PROVIDER / _MODEL  — override del operador
//   3. llm.default_provider / default_model (YAML) — base del install
//   4. primer provider con key, alfabetico — para que un usuario nuevo
//      entre y pueda usar el chat sin configurar nada
//
// El env var NO pisa lo que el usuario eligio: si lo hiciera, cambiar
// una variable para un deploy le borra la eleccion a todo el mundo sin
// avisar. Solo aplica cuando el usuario no tiene nada seteado.
//
// Sobre integridad referencial: los usuarios viven en ~/.config/neurox/
// users.json, no en SQLite (ver core/src/auth). No hay tabla de usuarios
// aqui, asi que `user_id` no puede tener FK. Si se borra un usuario del
// JSON, sus preferencias quedan huerfanas sin que nada lo detecte. La
// solucion de fondo es mover los usuarios a la DB; hasta entonces,
// `delete_for_missing_users` es la limpieza manual.

use chrono::Utc;
use sqlx::SqlitePool;

/// Una preferencia de LLM de un usuario. Los campos son opcionales a
/// proposito: la base del install queda DEFINIDA pero VACIA (sin fila),
/// y un usuario puede tener provider sin modelo explicito (usa el
/// `effective_model()` del provider).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UserLlmPref {
    pub provider_id: Option<String>,
    pub model: Option<String>,
}

impl UserLlmPref {
    /// Normaliza: cadenas vacias pasan a `None`. El front puede mandar
    /// `""` desde un `<select>` sin valor, y guardarlo como `Some("")`
    /// haria que `env::var("")` fallara mas tarde con un error confuso.
    pub fn normalize(provider_id: Option<&str>, model: Option<&str>) -> Self {
        fn clean(s: Option<&str>) -> Option<String> {
            s.map(str::trim)
                .filter(|t| !t.is_empty())
                .map(str::to_string)
        }
        Self {
            provider_id: clean(provider_id),
            model: clean(model),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.provider_id.is_none() && self.model.is_none()
    }
}

/// Resolucion final: que provider/modelo usa un usuario, y de donde sale.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedLlmPref {
    pub provider_id: Option<String>,
    pub model: Option<String>,
    /// Que fuente gano. Se expone por API para que el front pueda
    /// distinguir "el usuario eligio esto" de "esto vino del default".
    pub source: PrefSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrefSource {
    /// El usuario lo seteo explicitamente desde el front.
    User,
    /// Vino de NEUROX_DEFAULT_PROVIDER / _MODEL.
    Env,
    /// Vino de llm.default_provider / default_model del YAML.
    Config,
    /// Primer provider con key, alfabetico.
    AutoConfigured,
    /// No hay ningun provider con key: el chat no va a funcionar.
    NoneConfigured,
}

impl PrefSource {
    pub fn as_str(self) -> &'static str {
        match self {
            PrefSource::User => "user",
            PrefSource::Env => "env",
            PrefSource::Config => "config",
            PrefSource::AutoConfigured => "auto-configured",
            PrefSource::NoneConfigured => "none-configured",
        }
    }
}

pub async fn ensure_table(pool: &SqlitePool) -> Result<(), String> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS user_llm_prefs (
            user_id     TEXT PRIMARY KEY,
            provider_id TEXT,
            model       TEXT,
            updated_at  TEXT NOT NULL
        )",
    )
    .execute(pool)
    .await
    .map_err(|e| format!("failed to create user_llm_prefs table: {e}"))?;
    Ok(())
}

/// Lee la preferencia persistida del usuario. `Ok(None)` = nunca seteó.
pub async fn load(pool: &SqlitePool, user_id: &str) -> Result<Option<UserLlmPref>, String> {
    let row: Option<(Option<String>, Option<String>)> =
        sqlx::query_as("SELECT provider_id, model FROM user_llm_prefs WHERE user_id = ?")
            .bind(user_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("failed to load user_llm_prefs: {e}"))?;

    Ok(row.map(|(provider_id, model)| UserLlmPref {
        provider_id: provider_id.filter(|v| !v.is_empty()),
        model: model.filter(|v| !v.is_empty()),
    }))
}

/// Persiste la preferencia del usuario (upsert). Una pref vacia borra
/// la fila: "volver al default" es un estado valido, y dejarlo como
/// fila con NULLs haria indistinguible de "nunca seteó" en los reportes.
pub async fn save(pool: &SqlitePool, user_id: &str, pref: &UserLlmPref) -> Result<(), String> {
    if pref.is_empty() {
        return delete(pool, user_id).await;
    }
    let updated_at = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO user_llm_prefs (user_id, provider_id, model, updated_at)
         VALUES (?, ?, ?, ?)
         ON CONFLICT(user_id) DO UPDATE SET
             provider_id = excluded.provider_id,
             model       = excluded.model,
             updated_at  = excluded.updated_at",
    )
    .bind(user_id)
    .bind(pref.provider_id.as_deref())
    .bind(pref.model.as_deref())
    .bind(updated_at)
    .execute(pool)
    .await
    .map_err(|e| format!("failed to save user_llm_prefs: {e}"))?;
    Ok(())
}

/// Borra la preferencia: el usuario vuelve al default del install.
pub async fn delete(pool: &SqlitePool, user_id: &str) -> Result<(), String> {
    sqlx::query("DELETE FROM user_llm_prefs WHERE user_id = ?")
        .bind(user_id)
        .execute(pool)
        .await
        .map_err(|e| format!("failed to delete user_llm_prefs: {e}"))?;
    Ok(())
}

/// Resuelve la preferencia efectiva de un usuario.
///
/// `available` son los providers del store, y `is_configured` dice si
/// cada uno tiene key (lo determina el caller, que ya tiene la logica de
/// `api_key_env`). Se pasan como closures para no meter una dependencia
/// de `LlmProviderConfig` en este modulo.
///
/// Reglas, en orden:
/// - Si el usuario seteo algo Y ese provider tiene key, gana el usuario.
/// - Si el usuario seteo algo pero ese provider ya no tiene key (key
///   borrada, provider deshabilitado), NO se devuelve: caeria a un 401
///   en el primer mensaje del chat. Caemos al default y lo senalamos
///   con `source`, para que el front pueda avisar.
/// - Si no hay nada del usuario, aplica env, luego YAML, luego el
///   primer provider con key en orden alfabetico.
pub fn resolve_llm_pref(
    stored: Option<&UserLlmPref>,
    available: &[String],
    is_configured: &dyn Fn(&str) -> bool,
    config_default_provider: Option<&str>,
    config_default_model: Option<&str>,
) -> ResolvedLlmPref {
    // 1. Preferencia del usuario, si su provider sigue configurado.
    if let Some(pref) = stored {
        if let Some(pid) = pref.provider_id.as_deref() {
            if is_configured(pid) {
                return ResolvedLlmPref {
                    provider_id: Some(pid.to_string()),
                    model: pref.model.clone(),
                    source: PrefSource::User,
                };
            }
        }
    }

    // 2. Override del operador. Solo si el usuario no eligio: pisar una
    //    eleccion explicita por una variable de entorno seria invisible
    //    y destructivo.
    let env_provider = std::env::var("NEUROX_DEFAULT_PROVIDER").ok();
    let env_model = std::env::var("NEUROX_DEFAULT_MODEL").ok();
    let env_pick = env_provider
        .as_deref()
        .filter(|pid| available.iter().any(|a| a == pid) && is_configured(pid));
    if let Some(pid) = env_pick {
        return ResolvedLlmPref {
            provider_id: Some(pid.to_string()),
            model: env_model.filter(|m| !m.is_empty()),
            source: PrefSource::Env,
        };
    }

    // 3. Base del install (YAML).
    let yaml_pick = config_default_provider
        .filter(|pid| !pid.is_empty())
        .filter(|pid| available.iter().any(|a| a == pid))
        .filter(|pid| is_configured(pid));
    if let Some(pid) = yaml_pick {
        return ResolvedLlmPref {
            provider_id: Some(pid.to_string()),
            model: config_default_model
                .filter(|m| !m.is_empty())
                .map(str::to_string),
            source: PrefSource::Config,
        };
    }

    // 4. Primer provider con key, en orden alfabetico. Es lo que hace
    //    que un usuario nuevo, que nunca toco el selector, pueda igual
    //    usar el chat. `available` llega ordenado por el caller.
    if let Some(pid) = available.iter().find(|id| is_configured(id)) {
        return ResolvedLlmPref {
            provider_id: Some(pid.clone()),
            model: config_default_model
                .filter(|m| !m.is_empty())
                .map(str::to_string),
            source: PrefSource::AutoConfigured,
        };
    }

    ResolvedLlmPref {
        provider_id: None,
        model: None,
        source: PrefSource::NoneConfigured,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `resolve_llm_pref` lee `NEUROX_DEFAULT_PROVIDER` del entorno real
    /// y los tests corren en paralelo: el test del env var pisa el
    /// estado de los demas. El lock serializa los que tocan la env.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn ids(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    fn configured(set: &[&str]) -> impl Fn(&str) -> bool {
        let owned = set.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        move |id: &str| owned.iter().any(|s| s == id)
    }

    #[test]
    fn normalize_drops_blank_strings() {
        // Un <select> sin valor manda "". Guardarlo como Some("") haria
        // que el provider se llame "" y la busqueda falle confusamente.
        let p = UserLlmPref::normalize(Some("  "), Some(""));
        assert!(p.is_empty());
        let p = UserLlmPref::normalize(Some(" openai "), Some(" gpt-4o "));
        assert_eq!(p.provider_id.as_deref(), Some("openai"));
        assert_eq!(p.model.as_deref(), Some("gpt-4o"));
    }

    #[test]
    fn user_choice_wins_when_configured() {
        // El lock va tambien aca: este test asegura `source == User`, que
        // depende de que la rama del env no se haya colado antes.
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let r = resolve_llm_pref(
            Some(&UserLlmPref {
                provider_id: Some("openrouter".into()),
                model: Some("gpt-4o-mini".into()),
            }),
            &ids(&["minimax", "openrouter"]),
            &configured(&["minimax", "openrouter"]),
            Some("minimax"),
            Some("minimax-M3"),
        );
        assert_eq!(r.provider_id.as_deref(), Some("openrouter"));
        assert_eq!(r.source, PrefSource::User);
    }

    #[test]
    fn stale_user_choice_falls_back_when_key_gone() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // El usuario eligio openrouter pero la key se borro. Devolverlo
        // haria que el chat fallara con 401 en el primer mensaje.
        let r = resolve_llm_pref(
            Some(&UserLlmPref {
                provider_id: Some("openrouter".into()),
                model: None,
            }),
            &ids(&["minimax", "openrouter"]),
            &configured(&["minimax"]),
            Some("minimax"),
            None,
        );
        assert_eq!(r.provider_id.as_deref(), Some("minimax"));
        assert_ne!(r.source, PrefSource::User);
    }

    #[test]
    fn env_applies_only_when_user_has_no_choice() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        std::env::set_var("NEUROX_DEFAULT_PROVIDER", "openrouter");
        let avail = ids(&["minimax", "openrouter"]);
        let cfg = configured(&["minimax", "openrouter"]);

        // Sin eleccion del usuario: gana el env.
        let r = resolve_llm_pref(None, &avail, &cfg, Some("minimax"), None);
        assert_eq!(r.provider_id.as_deref(), Some("openrouter"));
        assert_eq!(r.source, PrefSource::Env);

        // Con eleccion del usuario: gana el usuario, el env NO pisa.
        let with_user = Some(&UserLlmPref {
            provider_id: Some("minimax".into()),
            model: None,
        });
        let r = resolve_llm_pref(with_user, &avail, &cfg, Some("minimax"), None);
        assert_eq!(r.provider_id.as_deref(), Some("minimax"));
        assert_eq!(r.source, PrefSource::User);

        std::env::remove_var("NEUROX_DEFAULT_PROVIDER");
    }

    #[test]
    fn falls_back_to_first_configured_alphabetical() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // Usuario nuevo, nada seteado, y el default del YAML no tiene
        // key. Tiene que poder usar el chat igual.
        let r = resolve_llm_pref(
            None,
            &ids(&["openrouter", "anthropic", "mistral"]),
            &configured(&["anthropic", "mistral"]),
            Some("openrouter"),
            None,
        );
        assert_eq!(r.provider_id.as_deref(), Some("anthropic"));
        assert_eq!(r.source, PrefSource::AutoConfigured);
    }

    #[test]
    fn none_configured_when_no_provider_has_key() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let r = resolve_llm_pref(
            None,
            &ids(&["minimax", "openai"]),
            &configured(&[]),
            Some("minimax"),
            None,
        );
        assert_eq!(r.provider_id, None);
        assert_eq!(r.source, PrefSource::NoneConfigured);
    }

    #[test]
    fn env_pointing_at_unknown_provider_is_ignored() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        std::env::set_var("NEUROX_DEFAULT_PROVIDER", "no-existe");
        let r = resolve_llm_pref(
            None,
            &ids(&["minimax"]),
            &configured(&["minimax"]),
            Some("minimax"),
            None,
        );
        assert_eq!(r.provider_id.as_deref(), Some("minimax"));
        assert_eq!(r.source, PrefSource::Config);
        std::env::remove_var("NEUROX_DEFAULT_PROVIDER");
    }

    #[tokio::test]
    async fn save_then_load_roundtrips() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        ensure_table(&pool).await.unwrap();

        assert_eq!(load(&pool, "u1").await.unwrap(), None);

        let pref = UserLlmPref {
            provider_id: Some("minimax".into()),
            model: Some("MiniMax-M3".into()),
        };
        save(&pool, "u1", &pref).await.unwrap();
        assert_eq!(load(&pool, "u1").await.unwrap(), Some(pref.clone()));

        // Upsert: el segundo save reemplaza, no duplica.
        let other = UserLlmPref {
            provider_id: Some("openai".into()),
            model: None,
        };
        save(&pool, "u1", &other).await.unwrap();
        assert_eq!(load(&pool, "u1").await.unwrap(), Some(other));
    }

    #[tokio::test]
    async fn prefs_are_isolated_per_user() {
        // El motivo de existir de esta tabla: dos usuarios eligen distinto.
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        ensure_table(&pool).await.unwrap();
        save(
            &pool,
            "user-a",
            &UserLlmPref {
                provider_id: Some("minimax".into()),
                model: None,
            },
        )
        .await
        .unwrap();
        save(
            &pool,
            "user-b",
            &UserLlmPref {
                provider_id: Some("openrouter".into()),
                model: Some("gpt-4o-mini".into()),
            },
        )
        .await
        .unwrap();

        let a = load(&pool, "user-a").await.unwrap().unwrap();
        let b = load(&pool, "user-b").await.unwrap().unwrap();
        assert_eq!(a.provider_id.as_deref(), Some("minimax"));
        assert_eq!(b.provider_id.as_deref(), Some("openrouter"));
        assert_eq!(b.model.as_deref(), Some("gpt-4o-mini"));
    }

    #[tokio::test]
    async fn saving_empty_pref_deletes_row() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        ensure_table(&pool).await.unwrap();
        save(
            &pool,
            "u1",
            &UserLlmPref {
                provider_id: Some("minimax".into()),
                model: None,
            },
        )
        .await
        .unwrap();
        save(&pool, "u1", &UserLlmPref::default()).await.unwrap();
        assert_eq!(load(&pool, "u1").await.unwrap(), None);
    }

    #[tokio::test]
    async fn ensure_table_is_idempotent() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        ensure_table(&pool).await.unwrap();
        ensure_table(&pool).await.unwrap();
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM user_llm_prefs")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }
}
