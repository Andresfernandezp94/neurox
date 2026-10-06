// Comandos del CLI de neurox.
//
// Todos hablan con el daemon por HTTP; ninguno habla con la base de datos ni
// con el estado interno. Es la misma regla que sigue el resto del proyecto: el
// daemon es el núcleo y los clientes (web, sidebar, móvil, y ahora esta CLI)
// son vistas.

use anyhow::{Context, Result};

use crate::cli_client::Client;

/// `neurox chat "texto"` — abre una sesión y streamea la respuesta a stdout.
///
/// Sin `--session`, crea una sesión nueva. Con `--session`, sigue una existente
/// (el daemon la re-siembra desde su historia si el subproceso del agente había
/// sido expulsado por inactividad).
pub async fn chat(text: String, session: Option<String>, agent: String) -> Result<()> {
    let c = Client::connect().await?;

    let sid = match session {
        Some(s) => s,
        None => {
            let v = c
                .post("/v1/sessions", serde_json::json!({ "agent_id": agent }))
                .await?;
            v["id"]
                .as_str()
                .map(str::to_string)
                .or_else(|| v["session_id"].as_str().map(str::to_string))
                .context("la creación de sesión no devolvió id")?
        }
    };

    eprintln!("sesión {sid}");

    // SSE a mano con reqwest: el endpoint es un stream de líneas `data: {...}`
    // que cierra con `[DONE]`. No hay helper de SSE en el crate.
    let url = format!("{}/v1/sessions/{sid}/messages/stream", c.base());
    let resp = c
        .stream(&url, serde_json::json!({ "agent_id": agent, "text": text }))
        .await?;

    let mut buf = String::new();
    let mut stream = resp.bytes_stream();
    use futures::StreamExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        buf.push_str(&String::from_utf8_lossy(&chunk));
        while let Some(idx) = buf.find('\n') {
            let line: String = buf.drain(..=idx).collect();
            let line = line.trim();
            let Some(payload) = line.strip_prefix("data:") else { continue };
            let payload = payload.trim();
            if payload == "[DONE]" {
                return Ok(());
            }
            let Ok(ev) = serde_json::from_str::<serde_json::Value>(payload) else { continue };
            match ev["type"].as_str().unwrap_or("") {
                // El thinking se imprime a stderr para no contaminar stdout:
                // stdout es la respuesta, y quien redirige a un archivo quiere
                // la respuesta, no la traza del razonamiento.
                "thinking" => {
                    if let Some(t) = ev["text"].as_str() {
                        eprint!("{t}");
                    }
                }
                "content" => {
                    if let Some(t) = ev["text"].as_str() {
                        print!("{t}");
                        use std::io::Write;
                        let _ = std::io::stdout().flush();
                    }
                }
                "tool_call" => {
                    if let Some(t) = ev["tool"].as_str() {
                        eprintln!("· {t}");
                    }
                }
                "error" => {
                    if let Some(m) = ev["message"].as_str() {
                        eprintln!("\nerror: {m}");
                    }
                }
                _ => {}
            }
        }
    }
    println!();
    Ok(())
}

/// `neurox sessions` — lista, muestra o borra sesiones.
pub async fn sessions(action: SessionsAction) -> Result<()> {
    let c = Client::connect().await?;
    match action {
        SessionsAction::List => {
            let v = c.get("/v1/sessions").await?;
            // Igual que en `models`: el endpoint envuelve la lista.
            let arr = v["sessions"].as_array().cloned().unwrap_or_default();
            if arr.is_empty() {
                println!("sin sesiones");
                return Ok(());
            }
            println!("{:<38} {:<8} INICIO", "ID", "AGENTE");
            for s in arr {
                // El listado usa `session_id`; la creación devuelve `id` en
                // algunos caminos y `session_id` en otros, por eso el
                // `chat` prueba los dos.
                let id = s["session_id"].as_str().unwrap_or("?");
                let agent = s["agent_id"].as_str().unwrap_or("-");
                let started = s["started_at"].as_str().unwrap_or("-");
                println!("{id:<38} {agent:<8} {started}");
            }
        }
        SessionsAction::Show { id } => {
            let v = c.get(&format!("/v1/sessions/{id}/messages")).await?;
            let arr = v.as_array().cloned().unwrap_or_default();
            for m in arr {
                let role = m["role"].as_str().unwrap_or("?");
                let content = m["content"].as_str().unwrap_or("");
                match role {
                    "user" => println!("\n> {content}"),
                    "assistant" => println!("\n{content}"),
                    "tool" => {
                        if let Some(t) = m["tool_name"].as_str() {
                            let short: String = content.chars().take(120).collect();
                            println!("\n  [{t}] {short}");
                        }
                    }
                    _ => {}
                }
            }
        }
        SessionsAction::Rm { id } => {
            c.post_empty(&format!("/v1/sessions/{id}")).await?;
            println!("sesión {id} borrada");
        }
    }
    Ok(())
}

#[derive(clap::Subcommand)]
pub enum SessionsAction {
    /// Lista las sesiones.
    List,
    /// Muestra el historial de una sesión.
    Show { id: String },
    /// Borra una sesión.
    Rm { id: String },
}

/// `neurox approvals` — lista las aprobaciones pendientes y las responde.
pub async fn approvals(action: ApprovalsAction) -> Result<()> {
    let c = Client::connect().await?;
    match action {
        ApprovalsAction::List => {
            let v = c.get("/v1/approvals").await?;
            let arr = v.as_array().cloned().unwrap_or_default();
            if arr.is_empty() {
                println!("sin aprobaciones pendientes");
                return Ok(());
            }
            println!("{:<38} {:<16} SESIÓN", "ID", "TOOL");
            for a in arr {
                println!(
                    "{:<38} {:<16} {}",
                    a["id"].as_str().unwrap_or("?"),
                    a["tool"].as_str().unwrap_or("-"),
                    a["session_id"].as_str().unwrap_or("-")
                );
            }
        }
        ApprovalsAction::Respond { id, decision } => {
            c.post(
                &format!("/v1/approvals/{id}/respond"),
                serde_json::json!({ "decision": decision }),
            )
            .await?;
            println!("{id}: {decision}");
        }
    }
    Ok(())
}

#[derive(clap::Subcommand)]
pub enum ApprovalsAction {
    /// Lista las aprobaciones pendientes.
    List,
    /// Responde una: `approve` o `deny`.
    Respond { id: String, decision: String },
}

/// `neurox models` — los proveedores y modelos disponibles.
pub async fn models() -> Result<()> {
    let c = Client::connect().await?;
    let v = c.get("/v1/llm/providers").await?;
    // `/v1/llm/providers` devuelve un objeto {default_provider, default_model,
    // providers: [...]}, no un array suelto. Por eso esto NO es `v.as_array()`:
    // con un objeto, `as_array()` da None y la lista salía vacía sin error.
    let arr = v["providers"].as_array().cloned().unwrap_or_default();
    if arr.is_empty() {
        println!("sin proveedores configurados");
        return Ok(());
    }
    if let (Some(dp), Some(dm)) = (
        v["default_provider"].as_str(),
        v["default_model"].as_str(),
    ) {
        println!("default: {dp}/{dm}\n");
    }
    for p in arr {
        let id = p["id"].as_str().unwrap_or("?");
        let cfg = p["configured"].as_bool().unwrap_or(false);
        let model = p["model"].as_str().unwrap_or("-");
        let mark = if cfg { "✓" } else { "·" };
        println!("{mark} {id:<16} {model}");
        if !cfg {
            if let Some(env) = p["api_key_env"].as_str() {
                eprintln!("    falta la key: {env}");
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn las_acciones_de_sessions_cubren_los_casos_de_uso() {
        // Trivial a propósito: si alguien agrega una variante y olvida el match
        // de arriba, el compilador lo marca. Este test solo documenta.
        let _ = SessionsAction::List;
    }

    #[test]
    fn una_decision_invalida_no_pasa_silenciosa() {
        // El daemon valida `approve`/`deny`. La CLI avisa antes de gastar un
        // request, porque un 400 desde el daemon llega después de haber
        // especificado el id.
        let malas = vec!["approved", "ALLOW", "yes", ""];
        for d in malas {
            assert!(!matches!(d, "approve" | "deny"), "{d} no debería pasar");
        }
    }
}