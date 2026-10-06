// Cliente HTTP del CLI de neurox.
//
// El daemon exige un JWT para casi todo (`auth.enabled: true` y `UserContext`
// como extractor en 10 handlers), así que la CLI necesita credenciales. En vez
// de pedir `--password` en cada llamada, usa el par access/refresh que el
// daemon ya expone:
//
//   1. Primer uso: login con usuario+contraseña → access JWT + refresh token.
//      El refresh se guarda en ~/.config/neurox/cli.json (0600).
//   2. Usos siguientes: canjea el refresh por un JWT nuevo, rotándolo.
//
// El refresh se rota en cada canje, igual que en la app Android, y aquí el
// riesgo es el mismo: si se pierde el fichero, hay que volver a escribir la
// contraseña. A cambio, tras 24 h el token de acceso caduca y la sesión
// sigue viva sin intervención, que es justo lo que un CLI de terminal
// necesita.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::io::IsTerminal;
use std::path::PathBuf;

/// Dónde viven la URL del daemon y el refresh token.
fn config_path() -> PathBuf {
    let mut p = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    p.push("neurox");
    p.push("cli.json");
    p
}

#[derive(Debug, Serialize, Deserialize)]
struct CliConfig {
    /// `http://host:puerto`. Por defecto el bind_addr del config local.
    #[serde(default = "default_base")]
    base: String,
    #[serde(default)]
    username: String,
    /// Opaque, de un solo uso. Nunca es el JWT de acceso.
    #[serde(default)]
    refresh_token: Option<String>,
}

/// Manual a propósito: `#[derive(Default)]` ignoraría
/// `#[serde(default = "default_base")]`, y dejaría `base: ""`.
///
/// Eso no es teórico: `load()` cae en el default cuando el fichero todavía no
/// existe, que es exactamente el primer uso. Con `base` vacía todas las URLs
/// salían sin host y la CLI fallaba con un error de URL en vez de pedir
/// login. Lo detectó `la_config_ausente_cae_en_defaults_en vez de panicar`.
impl Default for CliConfig {
    fn default() -> Self {
        Self {
            base: default_base(),
            username: String::new(),
            refresh_token: None,
        }
    }
}

fn default_base() -> String {
    "http://127.0.0.1:7878".into()
}

impl CliConfig {
    fn load() -> Self {
        std::fs::read_to_string(config_path())
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    /// Atomic-ish write: `0600` porque contiene una credencial. Se escribe a un
    /// temporal y se renombra, como el resto de stores del daemon, para que un
    /// fallo a medio escribir no deje el fichero truncado (eso sería peor que
    /// no tener config: la CLI pensaría que hay sesión y no la hay).
    fn save(&self) -> Result<()> {
        let path = config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(&tmp, &path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        }
        Ok(())
    }
}

/// Lo que devuelve `/v1/auth/login` y `/v1/auth/refresh-token`.
#[derive(Deserialize)]
struct AuthResponse {
    token: String,
    #[serde(default)]
    refresh_token: Option<String>,
}

pub struct Client {
    http: reqwest::Client,
    cfg: CliConfig,
    /// Access JWT. Vive SOLO en memoria, nunca en disco: caduca en 24 h y
    /// renovarlo es un POST a /v1/auth/refresh-token, así que no hay nada que
    /// gained guardándolo. El `Bearer` de cada petición va aquí, NO el
    /// refresh token: aquel no vale como credencial de acceso y el daemon
    /// devolvería 401.
    token: String,
}

impl Client {
    /// Construye el cliente y se asegura de tener un JWT válido, iniciando
    /// sesión si hace falta.
    ///
    /// `password` solo se pide si no hay refresh token utilizable.
    pub async fn connect() -> Result<Self> {
        let mut cfg = CliConfig::load();
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()?;

        // Intento 1: renovar con el refresh guardado.
        if let Some(rt) = cfg.refresh_token.clone() {
            match renew(&http, &cfg.base, &rt).await {
                Ok(auth) => {
                    if let Some(new) = auth.refresh_token {
                        cfg.refresh_token = Some(new);
                        let _ = cfg.save();
                    }
                    return Ok(Self {
                        http,
                        cfg,
                        token: auth.token,
                    });
                }
                Err(_) => {
                    // Caducado o revocado: se descarta y se vuelve a loguear.
                    cfg.refresh_token = None;
                }
            }
        }

        // Intento 2: login. `NEUROX_PASSWORD` va PRIMERO porque vale tanto con
        // terminal como sin ella (scripts, CI); el prompt es el respaldo.
        //
        // El orden importa: sin TTY y sin la variable, `rpassword` falla con un
        // "No such device or address" que no le dice nada a nadie. Se comprueba
        // antes de preguntar y el error dice qué hacer.
        let password = match std::env::var("NEUROX_PASSWORD") {
            Ok(p) if !p.is_empty() => p,
            _ => {
                if !std::io::stdin().is_terminal() {
                    bail!(
                        "no hay sesión de CLI y no hay terminal para pedir la contraseña.\n\
                         Ejecutá `neurox login` en una terminal, o:\n  \
                         NEUROX_PASSWORD=... neurox models"
                    );
                }
                rpassword::prompt_password("neurox password: ")?
            }
        };

        if cfg.username.is_empty() {
            cfg.username = if std::io::stdin().is_terminal() {
                let u = prompt("neurox user (default: andres.fernandez):")?;
                if u.is_empty() { "andres.fernandez".into() } else { u }
            } else {
                // Sin terminal no hay a quién preguntar: se usa el default,
                // que es el único usuario que el daemon crea al primer arranque.
                "andres.fernandez".into()
            };
        }
        let auth = login(&http, &cfg.base, &cfg.username, &password)
            .await
            .context("login failed")?;
        cfg.refresh_token = auth.refresh_token.clone();
        cfg.save()?;
        Ok(Self {
            http,
            cfg,
            token: auth.token,
        })
    }

    /// Igual que `connect` pero sin pedir nada por stdin. Para subcomandos
    /// programáticos (hooks, scripts) donde no hay terminal.
    ///
    /// Falla con un mensaje que dice exactamente qué hacer, en vez de
    /// colgarse esperando una tecla.
    pub async fn connect_noninteractive() -> Result<Self> {
        let mut cfg = CliConfig::load();
        // Se clona el refresh antes de construir el cliente: `cfg` se mueve
        // dentro del `Self` al final y el borrow de `as_deref()` viviría más
        // que eso.
        let rt = cfg
            .refresh_token
            .clone()
            .context(
                "no hay sesión de CLI. Ejecutá `neurox login` una vez \
                 (guarda un refresh token en ~/.config/neurox/cli.json)",
            )?;
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()?;
        let auth = renew(&http, &cfg.base, &rt).await?;
        if let Some(new) = auth.refresh_token {
            cfg.refresh_token = Some(new);
            let _ = cfg.save();
        }
        Ok(Self {
            http,
            cfg,
            token: auth.token,
        })
    }

    pub fn base(&self) -> &str {
        &self.cfg.base
    }

    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}{}", self.cfg.base, path);
        self.http.request(method, &url).bearer_auth(&self.token)
    }

    pub async fn get(&self, path: &str) -> Result<serde_json::Value> {
        self.request(reqwest::Method::GET, path)
            .send()
            .await?
            .json()
            .await
            .with_context(|| format!("GET {path}"))
    }

    pub async fn post(&self, path: &str, body: serde_json::Value) -> Result<serde_json::Value> {
        self.request(reqwest::Method::POST, path)
            .json(&body)
            .send()
            .await?
            .error_for_status()
            .with_context(|| format!("POST {path}"))?
            .json()
            .await
            .with_context(|| format!("POST {path}"))
    }

    /// POST sin cuerpo, para endpoints que no lo esperan (`/v1/auth/logout`).
    ///
    /// No toda respuesta tiene cuerpo JSON: `/v1/sessions/:id` devuelve un 200
    /// vacío al borrar. Por eso el `unwrap_or(Null)` en vez de `.json()?`.
    pub async fn post_empty(&self, path: &str) -> Result<serde_json::Value> {
        Ok(self
            .request(reqwest::Method::POST, path)
            .send()
            .await?
            .error_for_status()
            .with_context(|| format!("POST {path}"))?
            .json()
            .await
            .unwrap_or(serde_json::Value::Null))
    }

    /// POST de una URL completa, sin prefijar `base`. El stream de chat vive
    /// en `/v1/sessions/:id/messages/stream` y el id va en la ruta, así que se
    /// construye la URL entera en el caller.
    pub async fn stream(&self, url: &str, body: serde_json::Value) -> Result<reqwest::Response> {
        self.http
            .post(url)
            .bearer_auth(&self.token)
            .json(&body)
            .send()
            .await
            .with_context(|| format!("POST {url}"))?
            .error_for_status()
            .with_context(|| format!("POST {url}"))
    }
}

async fn login(
    http: &reqwest::Client,
    base: &str,
    username: &str,
    password: &str,
) -> Result<AuthResponse> {
    Ok(http
        .post(format!("{base}/v1/auth/login"))
        .json(&serde_json::json!({ "username": username, "password": password }))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?)
}

async fn renew(http: &reqwest::Client, base: &str, refresh: &str) -> Result<AuthResponse> {
    let resp = http
        .post(format!("{base}/v1/auth/refresh-token"))
        .json(&serde_json::json!({ "refresh_token": refresh, "client": "cli" }))
        .send()
        .await?
        .error_for_status()?;
    Ok(resp.json().await?)
}

fn prompt(msg: &str) -> Result<String> {
    use std::io::Write;
    print!("{msg}");
    std::io::stdout().flush()?;
    let mut s = String::new();
    std::io::stdin().read_line(&mut s)?;
    Ok(s.trim().to_string())
}

/// Login explícito, para dejar la CLI lista sin depender del primer comando.
pub async fn login_interactive() -> Result<()> {
    let cfg = CliConfig::load();
    let http = reqwest::Client::new();
    let username = if cfg.username.is_empty() {
        let u = prompt("neurox user (default: andres.fernandez):")?;
        if u.is_empty() {
            "andres.fernandez".into()
        } else {
            u
        }
    } else {
        cfg.username.clone()
    };
    let password = rpassword::prompt_password("neurox password: ")?;
    let auth = login(&http, &cfg.base, &username, &password)
        .await
        .context("login failed")?;
    let mut cfg = cfg;
    cfg.username = username;
    cfg.refresh_token = auth.refresh_token;
    cfg.save()?;
    println!("sesión guardada en {}", config_path().display());
    Ok(())
}

/// Cierra la sesión de la CLI: revoca el refresh y borra el fichero local.
pub async fn logout() -> Result<()> {
    let cfg = CliConfig::load();
    if let Some(rt) = cfg.refresh_token.as_deref() {
        // Revocar en el daemon es lo que importa: sin esto el token sigue
        // vivo 30 días aunque se borre el fichero de acá.
        let http = reqwest::Client::new();
        let _ = http
            .post(format!("{}/v1/auth/revoke-token", cfg.base))
            .json(&serde_json::json!({ "refresh_token": rt, "client": "cli" }))
            .send()
            .await;
    }
    let _ = std::fs::remove_file(config_path());
    println!("sesión cerrada");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_config_ausente_cae_en_defaults_en_vez_de_panicar() {
        // `load()` no puede fallar: si el fichero no existe o está corrupto,
        // la CLI tiene que arrancar igual y pedir login, no abortar.
        let c = CliConfig::default();
        assert_eq!(c.base, "http://127.0.0.1:7878");
        assert!(c.refresh_token.is_none());
    }

    #[test]
    fn el_refresh_sobrevive_a_una_ida_y_vuelta_de_json() {
        // Lo que se guarda es un refresh opaco, no un JWT: si este test se
        // rompe porque apareció un campo `token`, alguien está persistiendo el
        // access token en disco y hay que arreglarlo.
        let c = CliConfig {
            base: "http://x:1".into(),
            username: "u".into(),
            refresh_token: Some("abc".into()),
        };
        let back: CliConfig = serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
        assert_eq!(back.refresh_token.as_deref(), Some("abc"));
        let json = serde_json::to_string(&c).unwrap();
        assert!(!json.contains("\"token\""), "no debe haber campo `token`: {json}");
    }

    #[test]
    fn la_config_se_guarda_en_0600() {
        // Es una credencial. Si esto falla, el fichero quedó legible por otros.
        let dir = std::env::temp_dir().join(format!("neurox-cli-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("cli.json");
        let c = CliConfig {
            base: "http://x:1".into(),
            username: "u".into(),
            refresh_token: Some("secret".into()),
        };
        std::fs::write(&p, serde_json::to_vec_pretty(&c).unwrap()).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600)).unwrap();
            let m = std::fs::metadata(&p).unwrap().permissions().mode();
            assert_eq!(m & 0o777, 0o600);
        }
        std::fs::remove_dir_all(&dir).ok();
    }
}