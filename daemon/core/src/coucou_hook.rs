// `neurox coucou-hook <evento>` — el lado CLI de la integración con Coucou.
//
// Por qué una CLI y no código dentro del daemon: Coucou está construido
// alrededor de este modelo. Todos los agentes que soporta emiten eventos
// lanzando un proceso corto que escribe al socket (`coucou-hook`, que es
// exactamente esta forma), y el límite de proceso es lo que hace trivial su
// garantía de "nunca bloquear al agente": si el proceso se cuelga o muere,
// nadie se entera.
//
// Dentro del daemon eso no se puede garantizar igual. Publicar visibilidad
// sí (son bytes sueltos, se mandan en un `write`), pero una APROBACIÓN es
// bloqueante por definición: hay que esperar a que un humano cliquee. Si eso
// ocurriera en el hilo del bus, el `recv()` se detiene y neurox acumula lag
// en el canal de eventos para todos los demás.
//
// Con un subproceso, el daemon lanza `neurox coucou-hook PermissionRequest`,
// sigue sirviendo todo lo demás, y lee la decisión del stdout cuando llegue.
//
// ── El contrato ─────────────────────────────────────────────────────────────
//
//   neurox coucou-hook SessionStart
//   neurox coucou-hook UserPromptSubmit '{"prompt":"..."}'
//   neurox coucou-hook PermissionRequest '{"approval_id":"...","tool":"..."}'
//        ↓ escribe la línea JSON al socket
//        ↓ si es PermissionRequest, ESPERA y escribe la decisión en stdout
//
// stdout es la decisión y nada más: `approve`, `deny`, o nada. Coucou llama a
// eso "say nothing and the agent falls back", y para neurox el fallback es
// quedarse esperando su propia aprobación, que es exactamente lo correcto.

use anyhow::Result;
use std::path::PathBuf;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::cli_client::Client;

/// La island de Coucou solo mantiene el socket abierto mientras este plazo.
const CONNECT_TIMEOUT: Duration = Duration::from_millis(300);
/// Presupuesto de escritura. Un `write` de una línea a un socket local no
/// debería necesitar nada de esto.
const WRITE_TIMEOUT: Duration = Duration::from_secs(2);
/// Margen sobre el `DECISION_TIMEOUT` de Coucou (108 s): si Coucou agotó su
/// plazo, esta espera se rinde antes que él y neurox no queda colgado.
const DECISION_BUDGET: Duration = Duration::from_secs(110);

const AGENT_NAME: &str = "neurox";

/// Dónde busca la island. Mismo criterio que Coucou: `$XDG_RUNTIME_DIR` tiene
/// que ser nuestro y estar cerrado al grupo y a los demás.
fn socket_path() -> Option<PathBuf> {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| PathBuf::from(format!("/run/user/{}", uid())));

    use std::os::unix::fs::MetadataExt;
    let ours = std::fs::symlink_metadata(&dir)
        .map(|m| m.is_dir() && m.uid() == uid() && m.mode() & 0o077 == 0)
        .unwrap_or(false);

    ours.then(|| dir.join("coucou.sock"))
}

fn uid() -> u32 {
    // SAFETY: getuid no toma argumentos, no devuelve punteros y no falla.
    unsafe { libc::getuid() }
}

/// Escribe la línea y, si el evento pide decisión, espera a que Coucou
/// responda en la misma conexión.
///
/// Devuelve la decisión (`approve`/`deny`) o `None`. Los dos casos de `None`
/// son distintos a propósito:
///
///   * No hay Coucou. La island no existe y no hay nada que esperar.
///   * Coucou no decidió a tiempo. Se rinde para que neurox siga con su
///     propio flujo de aprobación.
///
/// En ambos casos Coucou no ve nada y neurox no se entera, que es lo que
/// corresponde a un adorno que nunca puede bloquear al agente.
pub async fn hook(event: &str, payload: serde_json::Value, session: Option<String>) -> Result<()> {
    let Some(path) = socket_path() else {
        return Ok(());
    };

    let mut body = payload;
    body["hook_event_name"] = serde_json::Value::String(event.to_string());
    body["coucou_agent"] = serde_json::Value::String(AGENT_NAME.to_string());
    if let Some(s) = session {
        body["session_id"] = serde_json::Value::String(s);
    }
    let mut line = body.to_string();
    line.push('\n');

    // SAFETY: conectar y escribir a un Unix socket no usa punteros ni comparte
    // memoria con otros hilos; los presupuestos son de tokio, no del bloqueo
    // del runtime.
    let connected = tokio::time::timeout(
        CONNECT_TIMEOUT,
        tokio::net::UnixStream::connect(&path),
    )
    .await;

    let Ok(Ok(mut stream)) = connected else {
        // Sin Coucou: se sale sin error. neurox no depende de esto.
        return Ok(());
    };

    if tokio::time::timeout(WRITE_TIMEOUT, stream.write_all(line.as_bytes()))
        .await
        .is_err()
    {
        return Ok(());
    }

    if event != "PermissionRequest" {
        return Ok(());
    }

    // Solo las aprobaciones esperan, y solo en este proceso: el daemon sigue
    // sirviendo streams mientras este hijo cuelga hasta 110 s.
    let decision = tokio::time::timeout(DECISION_BUDGET, read_decision(&mut stream)).await;
    let Ok(Ok(text)) = decision else {
        return Ok(());
    };

    match text.as_str() {
        "allow" | "always" => {
            apply(text == "allow", &body).await;
        }
        "deny" => apply(false, &body).await,
        // Cualquier otra cosa se ignora: el silencio es la respuesta segura.
        _ => {}
    }
    Ok(())
}

/// Lee una línea terminada en newline desde la conexión.
async fn read_decision(stream: &mut tokio::net::UnixStream) -> std::io::Result<String> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 256];
    loop {
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(idx) = buf.iter().position(|b| *b == b'\n') {
            buf.truncate(idx);
            break;
        }
    }
    Ok(String::from_utf8_lossy(&buf).trim().to_string())
}

/// Traduce la decisión de Coucou a la respuesta de neurox.
async fn apply(approved: bool, payload: &serde_json::Value) {
    let Some(id) = payload["approval_id"].as_str() else {
        return;
    };
    // Best-effort a propósito: si el daemon no está o ya venció la aprobación,
    // no hay nada que hacer y no vale la pena ensuciar el stream del chat.
    if let Ok(c) = Client::connect_noninteractive().await {
        let _ = c
            .post(
                &format!("/v1/approvals/{id}/respond"),
                serde_json::json!({
                    "decision": if approved { "approve" } else { "deny" }
                }),
            )
            .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_nombre_casa_el_validador_de_coucou() {
        // Coucou descarta lo que no case con `^[a-z0-9-]+$` y lo manda a la
        // pill de Claude Code. Si esto falla, los eventos de neurox acaban en
        // la pill equivocada en vez de en la suya.
        assert!(AGENT_NAME.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'));
        assert!(!AGENT_NAME.is_empty() && AGENT_NAME.len() <= 24);
    }

    #[tokio::test]
    async fn sin_socket_no_hay_error() {
        // El caso normal con Coucou desinstalado: tiene que salir limpio, no
        // propagar un error que rompería el stream del chat.
        let r = hook("SessionStart", serde_json::json!({}), None).await;
        assert!(r.is_ok());
    }

    #[tokio::test]
    async fn una_aprobacion_sin_socket_no_rompe_nada() {
        // Esta es la que importa: el camino de error del que nunca debe haber
        // resultado visible para neurox.
        let r = hook(
            "PermissionRequest",
            serde_json::json!({"approval_id":"x","tool":"shell"}),
            Some("s".into()),
        )
        .await;
        assert!(r.is_ok());
    }

    #[test]
    fn el_presupuesto_de_decision_supera_al_de_coucou() {
        // Coucou corta a los 108 s. Si neurox esperara menos, volvería a
        // preguntar mientras el usuario todavía está decidiendo: dos
        // peticiones para una decisión.
        assert!(DECISION_BUDGET.as_secs() > 108, "hay que esperar MÁS que Coucou");
    }
}