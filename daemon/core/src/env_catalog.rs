//! Catálogo de variables de entorno que neurox reconoce.
//!
//! `GET /v1/env` antes devolvía solo las keys que ya estaban escritas en el
//! archivo `env`. Eso hace imposible agregar una variable desde la UI: no
//! hay forma de que aparezca algo que nadie puso a mano. El catálogo vive
//! acá, en el daemon, para que el front no tenga que duplicar la lista
//! (y para que las dos copias no se desincronicen).
//!
//! La organization por responsabilidad no es decorativa: `Auth` va
//! aislada porque la password del admin viaja en el mismo archivo y con el
//! mismo 0600 que las keys de providers. Si algún día se agrega un listado
//! bulk de env vars, esta es la categoría que tiene que quedar excluida, y
//! para que esa exclusión sea natural la separación tiene que existir antes.
//!
//! ## Por qué el watcher no quita variables
//!
//! `std::env::remove_var` es `unsafe` desde Rust 1.65: libc mantiene su
//! propia copia de `environ` y no hay forma segura de re-sincronizarla sin
//! reiniciar. Por eso el watcher es estrictamente aditivo. Borrar una
//! variable es un acto explícito vía `DELETE /v1/env/:key`.
//!
//! ## Formato
//!
//! El archivo `env` se parsea como `KEY=valor` pelado. NO se interpretan
//! comillas ni `${...}` de dotenv: un valor entre comillas guarda las
//! comillas literales en el valor y la key falla en el provider sin error
//! visible. Los `default_value` de este catálogo son valores reales que se
//! escriben tal cual, sin quoting.

/// Categoría de una variable. El orden de las variantes es el orden en que
/// se las presenta al operador: de la más frecuente a la más sensible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EnvCategory {
    /// Configuración del propio daemon. Sin secretos.
    Runtime,
    /// Default de LLM: override del operador.
    DefaultLlm,
    /// Credenciales. Aislada a propósito.
    Auth,
    /// Plugins y tool engine.
    Integrations,
    /// Las aporta el host. No las escribimos nunca: se listan para que el
    /// operador sepa qué puede llegar a interferir, en modo lectura.
    Infrastructure,
}

impl EnvCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            EnvCategory::Runtime => "runtime",
            EnvCategory::DefaultLlm => "default-llm",
            EnvCategory::Auth => "auth",
            EnvCategory::Integrations => "integrations",
            EnvCategory::Infrastructure => "infrastructure",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            EnvCategory::Runtime => "Runtime",
            EnvCategory::DefaultLlm => "Default de LLM",
            EnvCategory::Auth => "Auth",
            EnvCategory::Integrations => "Integraciones",
            EnvCategory::Infrastructure => "Infraestructura",
        }
    }

    /// Descripción que se muestra en la UI, para que un operador no tenga
    /// que leer el código para saber qué hace la variable.
    pub fn description(self) -> &'static str {
        match self {
            EnvCategory::Runtime => "Configuración del propio daemon. Sin secretos.",
            EnvCategory::DefaultLlm => {
                "Override del operador. Aplica solo cuando el usuario no eligió \
                 provider/modelo: pisar una elección explícita sería invisible \
                 y destructivo."
            }
            EnvCategory::Auth => {
                "Aislada a propósito: la password del admin nunca debe viajar \
                 junto a las keys de providers."
            }
            EnvCategory::Integrations => "Plugins y tool engine.",
            EnvCategory::Infrastructure => {
                "Las aporta el host (HOME, PATH, XDG_*). No las escribe neurox: \
                 se listan para saber qué puede interferir."
            }
        }
    }
}

/// Una variable del catálogo.
#[derive(Debug, Clone, Copy)]
pub struct EnvVarSpec {
    pub key: &'static str,
    pub category: EnvCategory,
    /// Qué hace, en una línea.
    pub description: &'static str,
    /// Valor con el que arranca si no se setea. `None` = sin default
    /// (hay un default implícito en el código).
    pub default_value: Option<&'static str>,
    /// Variable que guarda un secreto: la UI la muestra como password y
    /// nunca devuelve el valor.
    pub sensitive: bool,
    /// Solo lectura. Las de infraestructura las pone el host; escribirla
    /// desde la UI tendría efecto hasta el próximo reinicio y después
    /// perdería la battle contra el valor real del entorno.
    pub read_only: bool,
}

/// Provider keys que se administran desde la tab Providers (cada provider
/// declara su `api_key_env` y ahí se edita). Se listan acá en modo
/// lectura para que el operador sepa que existen y dónde viven, sin
/// duplicar el lugar de escritura.
pub fn provider_api_keys() -> &'static [EnvVarSpec] {
    static KEYS: &[EnvVarSpec] = &[
        spec_sensitive_ro("MINIMAX_API_KEY", "API key de MiniMax."),
        spec_sensitive_ro("ANTHROPIC_API_KEY", "API key de Anthropic (Claude)."),
        spec_sensitive_ro("OPENAI_API_KEY", "API key de OpenAI."),
        spec_sensitive_ro("XAI_API_KEY", "API key de xAI (Grok)."),
        spec_sensitive_ro("GEMINI_API_KEY", "API key de Google Gemini."),
        spec_sensitive_ro("MISTRAL_API_KEY", "API key de Mistral."),
        spec_sensitive_ro("OPENROUTER_API_KEY", "API key de OpenRouter."),
        spec_sensitive_ro("OPENCODE_API_KEY", "API key de opencode Zen."),
    ];
    KEYS
}

const fn spec(
    key: &'static str,
    category: EnvCategory,
    description: &'static str,
    sensitive: bool,
    default_value: Option<&'static str>,
) -> EnvVarSpec {
    EnvVarSpec {
        key,
        category,
        description,
        default_value,
        sensitive,
        read_only: false,
    }
}

/// Key de provider: sensible y solo lectura. El lugar de escritura es la
/// tab Providers, que ya la edita via el `api_key_env` del provider.
const fn spec_sensitive_ro(key: &'static str, description: &'static str) -> EnvVarSpec {
    EnvVarSpec {
        key,
        category: EnvCategory::DefaultLlm,
        description,
        default_value: None,
        sensitive: true,
        read_only: true,
    }
}

const fn spec_ro(
    key: &'static str,
    category: EnvCategory,
    description: &'static str,
    default_value: Option<&'static str>,
) -> EnvVarSpec {
    EnvVarSpec {
        key,
        category,
        description,
        default_value,
        sensitive: false,
        read_only: true,
    }
}

/// Variables administrables desde la UI: todo menos las keys de providers
/// (que viven en su tab) y las de infraestructura (solo lectura).
///
/// Cada entrada se verificó contra el código: o se lee con
/// `std::env::var` en el daemon, o se documenta en el unit de systemd.
pub fn manageable() -> &'static [EnvVarSpec] {
    static VARS: &[EnvVarSpec] = &[
        // ── Runtime ──
        spec("NEUROX_WORKSPACE", EnvCategory::Runtime, "Raíz del workspace.", false, None),
        spec("NEUROX_ENV_FILE", EnvCategory::Runtime, "Ruta alterna del archivo env (default ~/.config/neurox/env).", false, None),
        spec("NEUROX_MAX_TOOL_ITERATIONS", EnvCategory::Runtime, "Tope de iteraciones de tools por mensaje. `0` = sin tope, para que el agente se autogestione.", false, Some("50")),
        spec("NEUROX_SESSION_STREAM_TIMEOUT_SECS", EnvCategory::Runtime, "Timeout de streaming de una sesión, en segundos.", false, Some("180")),
        spec("NEUROX_LOG_FORMAT", EnvCategory::Runtime, "Formato del log. `json` para salida estructurada.", false, None),
        spec("RUST_LOG", EnvCategory::Runtime, "Nivel de log por crate.", false, None),
        spec("NEUROX_REGISTRY", EnvCategory::Runtime, "URL del registro de agentes.", false, None),
        // ── Default de LLM (override del operador) ──
        spec("NEUROX_DEFAULT_PROVIDER", EnvCategory::DefaultLlm, "Provider por defecto. Si no está, usa el primer provider con key (alfabético).", false, None),
        spec("NEUROX_DEFAULT_MODEL", EnvCategory::DefaultLlm, "Modelo por defecto. Vacío = el `model` del provider elegido.", false, None),
        spec("NEUROX_MODELS_DIR", EnvCategory::DefaultLlm, "Directorio de modelos GGUF locales.", false, Some("~/models")),
        // ── Auth ──
        spec("NEUROX_ADMIN_PASSWORD", EnvCategory::Auth, "Password del usuario admin inicial. Solo se lee si no existe `users.json`.", true, None),
        // ── Integraciones ──
        spec("NEUROX_IDENTITY_DIR", EnvCategory::Integrations, "Directorio de identidad de los agentes.", false, None),
        spec("NEUROX_TODO_DIR", EnvCategory::Integrations, "Directorio del store de tareas (todos).", false, None),
        spec("NEUROX_TOOLS_ALLOWLIST", EnvCategory::Integrations, "Tools habilitadas, separadas por coma. Vacío = todas.", false, None),
    ];
    VARS
}

/// Variables que las aporta el host. Se listan en modo lectura para que un
/// operador que '+' una variable tipo PATH sepa que el valor del host gana.
pub fn infrastructure() -> &'static [EnvVarSpec] {
    static VARS: &[EnvVarSpec] = &[
        spec_ro("HOME", EnvCategory::Infrastructure, "Home del usuario. Default de varias rutas.", None),
        spec_ro("PATH", EnvCategory::Infrastructure, "Ruta de ejecutables. Bajo systemd es mínimo: usá rutas absolutas en `command`.", None),
        spec_ro("XDG_CONFIG_HOME", EnvCategory::Infrastructure, "Base de configuración XDG.", None),
        spec_ro("XDG_DATA_HOME", EnvCategory::Infrastructure, "Base de datos XDG.", None),
        spec_ro("XDG_RUNTIME_DIR", EnvCategory::Infrastructure, "Base de runtime XDG (sockets).", None),
    ];
    VARS
}

/// El catálogo completo, incluidas las keys de providers en solo lectura.
pub fn all() -> Vec<EnvVarSpec> {
    let mut out = manageable().to_vec();
    out.extend_from_slice(infrastructure());
    out.extend_from_slice(provider_api_keys());
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn keys_are_unique_across_the_catalog() {
        // Un duplicado haría que la UI muestre dos cards para la misma
        // variable, y que el `set` de una no se refleje en la otra.
        let all = all();
        let mut seen = HashSet::new();
        for v in &all {
            assert!(seen.insert(v.key), "duplicate key in catalog: {}", v.key);
        }
    }

    #[test]
    fn keys_are_valid_env_names() {
        // Si el catálogo declarara algo que `validate_key` rechaza, la
        // UI ofrecería una variable que el PUT siempre devuelve 400.
        for v in all() {
            assert!(
                crate::environments::validate_key(v.key).is_ok(),
                "catalog key rejected by validate_key: {}",
                v.key
            );
        }
    }

    #[test]
    fn provider_keys_are_read_only_and_sensitive() {
        // Se listan pero no se editan acá: el lugar de escritura es la tab
        // Providers. Editar desde los dos lados haría que una sobreescriba
        // a la otra sin avisar.
        for v in provider_api_keys() {
            assert!(v.read_only, "provider key must be read-only: {}", v.key);
            assert!(v.sensitive, "provider key must be sensitive: {}", v.key);
        }
    }

    #[test]
    fn auth_is_its_own_category() {
        // La razón de aislar Auth: es lo que permite excluirlo de un
        // listado bulk sin tener que filtrar nombres uno por uno.
        let auth: Vec<_> = manageable().iter().filter(|v| v.category == EnvCategory::Auth).collect();
        assert!(!auth.is_empty(), "auth category must not be empty");
        for v in &auth {
            assert!(v.sensitive, "auth var must be sensitive: {}", v.key);
        }
        // Y ninguna key de provider debe haber colado en Auth.
        for v in auth {
            assert!(
                !provider_api_keys().iter().any(|p| p.key == v.key),
                "provider key leaked into auth: {}",
                v.key
            );
        }
    }

    #[test]
    fn infrastructure_is_read_only() {
        for v in infrastructure() {
            assert!(v.read_only, "infrastructure var must be read-only: {}", v.key);
        }
    }

    #[test]
    fn defaults_are_not_sensitive() {
        // Un default sensible escribiría un secreto al disco en el momento
        // de aplicar el catálogo.
        for v in manageable() {
            if let Some(d) = v.default_value {
                assert!(
                    !v.sensitive,
                    "sensitive var {} must not have a default_value",
                    v.key
                );
                assert!(!d.is_empty(), "default of {} must not be empty", v.key);
            }
        }
    }

    #[test]
    fn every_spec_has_a_description() {
        // Sin descripción, el operador tiene que abrir el código para
        // saber qué hace la variable.
        for v in all() {
            assert!(!v.description.trim().is_empty(), "{} has no description", v.key);
        }
    }

    /// El catálogo promete defaults. Si el código cambia su fallback y el
    /// catálogo no, la UI muestra un valor que el daemon nunca usa: el
    /// operador aplica el default del catálogo, cree que quedo en 10, y
    /// el daemon sigue en 180. Estos asserts atan ambos lados.
    #[test]
    fn defaults_match_the_daemon_fallbacks() {
        let by_key = |k: &str| {
            all()
                .into_iter()
                .find(|v| v.key == k)
                .unwrap_or_else(|| panic!("{k} missing from catalog"))
        };
        assert_eq!(
            by_key("NEUROX_MAX_TOOL_ITERATIONS").default_value,
            Some(crate::router::max_tool_iterations().to_string()).as_deref()
        );
        assert_eq!(
            by_key("NEUROX_SESSION_STREAM_TIMEOUT_SECS").default_value,
            Some(crate::router::session_stream_timeout_secs().to_string()).as_deref()
        );
        assert_eq!(
            by_key("NEUROX_MODELS_DIR").default_value,
            Some(crate::local_models::DEFAULT_MODELS_DIR)
        );
    }
}