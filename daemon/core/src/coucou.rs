// Bridge a Coucou: publica la actividad de las sesiones de neurox en la
// "isla" de Coucou, el compañero de escritorio que vive en el notch del Mac o
// en el borde superior de la pantalla en Linux/Windows.
//
// Qué es Coucou: https://github.com/Louis-CFM/coucou. No es un servicio al que
// neurox le pida nada, ni al que haya que registrarse. Es un LECTOR pasivo: si
// neurox no escribe, Coucou no sabe que neurox existe.
//
// El transporte es un socket Unix en `$XDG_RUNTIME_DIR/coucou.sock` y el
// formato son líneas JSON terminadas en newline. Sin API keys, sin cuentas,
// sin red: por eso el modo de operación no necesita configurar nada.
//
// ── La regla que domina el diseño ───────────────────────────────────────────
//
// Coucou repite en su propio código (hook/src/main.rs) que NUNCA debe
// bloquear al agente. Acá el riesgo es el inverso: neurox es el daemon que
// atiende el chat, y un socket que acepta la conexión y después deja de leer
// podría frenar el stream entero. Por eso:
//
//   * Conectar tiene presupuesto (300 ms). Sin socket ⇒ no hay nada que hacer.
//   * Escribir tiene presupuesto (2 s) y se hace en una tarea aparte, así que
//     ni siquiera el escritura bloquea el runtime: el `select!` la abandona.
//   * Nunca se reintenta en bucle cerrado. Si Coucou no está, no se está.
//
// O sea: neurox funciona exactamente igual con Coucou corriendo, apagado, o
// simplemente no instalado. Este módulo nunca es un requisito.
//
// ── Por qué el bus y no el stream SSE ───────────────────────────────────────
//
// Podríamos suscribirnos al endpoint de cada sesión, pero `Event::
// SessionStarted` solo lo emite el dispatcher cuando nace la sesión, y un
// suscriptor HTTP que se crea después no lo ve. Suscribiéndonos al bus
// (`broadcast::Sender<Event>`) vemos el ciclo completo: alta, prompts, tools y
// cierre, sin polling y sin estado propio que mantener sincronizado.

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;

use serde_json::json;
use tokio::io::AsyncWriteExt;
use tokio::sync::broadcast::Receiver;
use uuid::Uuid;

use crate::events::Event;

/// El nombre de la pill. Coucou lo valida contra `^[a-z0-9-]+$` y descarta
/// lo que no case, mandando el evento a la pill de Claude Code. "neurox" casa.
const AGENT_NAME: &str = "neurox";

/// Presupuesto para CONSEGUIR la conexión. Es lo que hace el relay de Coucou.
const CONNECT_TIMEOUT: Duration = Duration::from_millis(300);
/// Presupuesto para escribir una línea. No debería agotarse nunca: son unos
/// pocos cientos de bytes a un socket local.
const WRITE_TIMEOUT: Duration = Duration::from_secs(2);

/// `$XDG_RUNTIME_DIR/coucou.sock`, o `/run/user/<uid>/coucou.sock` si la
/// variable no está (un servicio arrancado con el entorno recortado).
///
/// Se comprueba que el directorio sea nuestro y esté cerrado al grupo y a los
/// demás (`mode & 0o077 == 0`), que es lo que guarantees que nadie más se
/// cuelgue en nuestro socket. Mismo criterio que el relay de Coucou.
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

/// El uid real del proceso. `getuid` no puede fallar ni tocar memoria, y
/// `libc` ya es dependencia del crate.
fn uid() -> u32 {
    // SAFETY: getuid no toma argumentos, no devuelve punteros y no puede
    // fallar. Es la única llamada unsafe del módulo, y es inocua.
    unsafe { libc::getuid() }
}

fn format_uuid(id: Uuid) -> String {
    id.to_string()
}

/// Estado por sesión, para no repetirle a Coucou lo que ya sabe.
///
/// Coucou NO deduplica: cada línea que llega cambia el estado de la pill. Sin
/// este set, neurox le reporta el mismo cambio tres veces y se nota (una
/// fila parpadea, una Sound se dispara triple).
///
/// Solo se recuerda lo que de verdad se repite. `PreToolUse`/`PostToolUse` NO
/// se guardan: neurox corre las tools en paralelo y una de ellas leyó 3
/// archivos a la vez, así que tres `PostToolUse` con el mismo instante son tres
/// tools distintas y legítimas. Filtrarlos sería mentir sobre el trabajo real.
#[derive(Default)]
pub struct Seen {
    /// Turno ya anunciado: el primer `Thinking` de cada turno.
    prompted: HashSet<Uuid>,
    /// El `Stop` del turno ya se mandó (o se suprimió).
    stopped: HashSet<Uuid>,
}

/// Traduce un evento de neurox al payload canónico de Coucou.
///
/// Devuelve `None` para lo que Coucou no entiende o para lo que ya se dijo.
///
/// `Content` y `Thinking` llegan POR CHUNK: decenas por respuesta. Coucou no
/// tiene evento para contenido, así que el texto de la respuesta no se manda;
/// el prompt se anuncia una vez (primer `Thinking`) y las tools ya se reportan
/// en `PreToolUse`.
fn to_coucou(ev: &Event, seen: &mut Seen) -> Option<serde_json::Value> {
    let base = |event: &str, session: Uuid| {
        json!({
            "hook_event_name": event,
            "session_id": format_uuid(session),
            "coucou_agent": AGENT_NAME,
        })
    };

    match ev {
        Event::SessionStarted { session_id, agent_id } => {
            let mut v = base("SessionStart", *session_id);
            v["agent_id"] = json!(agent_id);
            Some(v)
        }
        // El thinking marca que neurox aceptó el prompt y está trabajando: es
        // lo que en Coucou pone la isla en "pensando". Como llega por chunks,
        // solo el primero del turno cuenta.
        Event::Thinking { session_id, .. } => seen
            .prompted
            .insert(*session_id)
            .then(|| {
                // Turno nuevo: el `Stop` del anterior ya se mandó, así que
                // este turno necesita poder cerrar el suyo.
                seen.stopped.remove(session_id);
                base("UserPromptSubmit", *session_id)
            }),
        Event::ToolCall {
            session_id, tool, ..
        } => {
            let mut v = base("PreToolUse", *session_id);
            v["tool_name"] = json!(tool);
            Some(v)
        }
        Event::ToolResult {
            session_id, tool, ..
        } => {
            let mut v = base("PostToolUse", *session_id);
            v["tool_name"] = json!(tool);
            Some(v)
        }
        Event::Done { session_id, .. } => seen
            .stopped
            .insert(*session_id)
            .then(|| {
                // El turno cerró: el próximo Thinking vuelve a ser un prompt
                // nuevo, no un chunk más del mismo.
                seen.prompted.remove(session_id);
                base("Stop", *session_id)
            }),
        // `Done` y `Error` cierran el mismo turno: el forwarder deja el stream
        // abierto a propósito tras un error (ver http.rs, "D2 fix") para no
        // perder el `ToolResult` que llega justo después. Mapear los dos a
        // `Stop` mandaba el cierre dos veces; solo el que cierra primero vale.
        // Coucou no distingue error de fin: `StopFailure` solo cambia el color.
        Event::Error { session_id, .. } => session_id.filter(|s| seen.stopped.insert(*s))
            .map(|s| {
                seen.prompted.remove(&s);
                base("Stop", s)
            }),
        Event::SessionEnded { session_id, .. } => {
            // La sesión se va: se olvida el turno, por si el id se reutiliza.
            seen.prompted.remove(session_id);
            seen.stopped.remove(session_id);
            Some(base("SessionEnd", *session_id))
        }
        _ => None,
    }
}

/// Escribe una línea al socket. Un fallo aquí no es un error de neurox: Coucou
/// puede haberse cerrado entre medio. Se traga y sigue.
async fn send(line: &str, path: &PathBuf) -> bool {
    let write = async {
        let mut stream = match tokio::time::timeout(CONNECT_TIMEOUT, tokio::net::UnixStream::connect(path))
            .await
        {
            Ok(Ok(s)) => s,
            _ => return false,
        };
        // Un Write de una línea a un socket local no debería necesitar más,
        // pero el presupuesto va igual: el socket es de otro proceso.
        tokio::time::timeout(WRITE_TIMEOUT, stream.write_all(line.as_bytes())).await.is_ok()
    };

    write.await
}

/// Traduce un evento y lo publica. Expuesta aparte para poder probarla sin
/// socket.
pub async fn publish(ev: &Event, path: &PathBuf, seen: &mut Seen) {
    let Some(payload) = to_coucou(ev, seen) else { return };
    let mut line = payload.to_string();
    line.push('\n');
    send(&line, path).await;
}

/// Ruta del propio binario, para relanzarse como hook.
///
/// `std::env::current_exe()` es lo correcto y no una constante: el daemon
/// corre como servicio de systemd desde `~/.local/bin`, pero en desarrollo
/// desde `target/release`, y un path fijo fallaría en uno de los dos casos.
fn self_exe() -> Option<PathBuf> {
    std::env::current_exe().ok()
}

/// Publica una APROBACIÓN y espera la decisión del usuario.
///
/// A diferencia de `publish`, esto no puede ser un `write`: una decisión
/// humana tarda, y esperar aquí detendría el `recv()` del bus, que es
/// compartido. Así que corre en su propia tarea y el bucle sigue.
///
/// El subproceso es `neurox coucou-hook PermissionRequest`, que es este mismo
/// binario con otro subcomando. El límite de proceso es lo que hace segura la
/// garantía: si el hook se cuelga o el usuario nunca contesta, neurox sigue
/// sirviendo y su propia aprobación sigue esperándolo por su canal.
pub fn spawn_approval_bridge(request: &crate::approval::ApprovalRequest) {
    let Some(exe) = self_exe() else {
        tracing::debug!("coucou: no se puede localizar el propio binario, sin aprobaciones");
        return;
    };

    // Todo se copia a `String` antes del spawn: la tarea vive en el runtime y
    // no puede prête del `request` del bus.
    let payload_arg = serde_json::json!({
        "approval_id": request.id.to_string(),
        "tool_name": request.tool,
        "tool_input": request.args,
        "prompt": format!("Allow {}?", request.tool),
    })
    .to_string();
    let session_arg = request.session_id.to_string();

    tokio::spawn(async move {
        let out = tokio::process::Command::new(exe)
            .arg("coucou-hook")
            .arg("PermissionRequest")
            .arg(&payload_arg)
            .arg("--session")
            .arg(&session_arg)
            .stdin(std::process::Stdio::null())
            .output()
            .await;

        match out {
            Ok(o) if o.status.success() => tracing::debug!("coucou: aprobación resuelta"),
            Ok(o) => tracing::debug!("coucou: el hook salió con {:?}", o.status),
            Err(e) => tracing::debug!("coucou: no se pudo lanzar el hook: {e}"),
        }
    });
}

/// Tarea de fondo. Se queda escuchando el bus mientras el daemon viva.
///
/// Si Coucou no está instalado, el primer `send` falla y el bucle sigue
/// escuchando: cuando Coucou arranque más tarde (que es el caso normal: se
/// abre cuando hay una sesión) empieza a recibir sin que neurox haga nada.
pub async fn run(mut rx: Receiver<Event>) {
    let Some(path) = socket_path() else {
        // Sin directorio de runtime propio no hay socket posible. No es un
        // error: neurox sigue igual, solo que sin Coucou.
        tracing::debug!("coucou: no hay XDG_RUNTIME_DIR utilizable, bridge inactivo");
        return;
    };

    tracing::debug!("coucou: bridge escuchando en {}", path.display());

    let mut seen = Seen::default();
    loop {
        match rx.recv().await {
            Ok(ev) => {
                // Las aprobaciones salen por el hook en un subproceso, no por
                // el socket directo: el hook espera la decisión del usuario y
                // eso no puede bloquear este bucle.
                if let Event::ApprovalRequest { request } = &ev {
                    spawn_approval_bridge(request);
                    continue;
                }
                publish(&ev, &path, &mut seen).await;
            }
            // Lagged: nos pasamos de la capacidad del canal (1024). Coucou es
            // un adorno, no una fuente de verdad: mejor perder eventos sueltos
            // que retencer el bus para neurox.
            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                tracing::debug!("coucou: {n} eventos perdidos por lag, sigo");
            }
            // Cerrado: el bus se apagó, el daemon está bajando.
            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sid() -> Uuid {
        Uuid::new_v4()
    }

    #[test]
    fn el_nombre_del_agente_es_valido_para_coucou() {
        // Coucou descarta lo que no case con esta expresion (hooks.ts:33) y lo
        // manda a la pill de Claude Code. Si esto falla, neurox no tiene pill.
        assert!(AGENT_NAME.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'));
        assert!(!AGENT_NAME.is_empty() && AGENT_NAME.len() <= 24);
    }

    #[test]
    fn el_turno_completo_mapea_a_los_eventos_canónicos() {
        let s = sid();
        let seen: Vec<String> = [
            Event::SessionStarted {
                session_id: s,
                agent_id: "default".into(),
            },
            Event::Thinking {
                session_id: s,
                text: "razonando".into(),
                seq: 1,
            },
            Event::ToolCall {
                session_id: s,
                tool: "read_file".into(),
                args: json!({}),
                iteration: 1,
                call_id: "c1".into(),
                seq: 2,
            },
            Event::ToolResult {
                session_id: s,
                tool: "read_file".into(),
                result: "ok".into(),
                iteration: 1,
                call_id: "c1".into(),
                seq: 3,
            },
            Event::Done {
                session_id: s,
                text: "listo".into(),
            },
            Event::SessionEnded {
                session_id: s,
                summary: None,
            },
        ]
        .iter()
        .filter_map(|e| to_coucou(e, &mut Seen::default()))
        .map(|v| v["hook_event_name"].as_str().unwrap().to_string())
        .collect();

        assert_eq!(
            seen,
            vec![
                "SessionStart",
                "UserPromptSubmit",
                "PreToolUse",
                "PostToolUse",
                "Stop",
                "SessionEnd"
            ]
        );
    }

    /// Convierte una ristra de eventos en los nombres que Coucou recibiría.
    fn traza(events: &[Event]) -> Vec<String> {
        let mut seen = Seen::default();
        events
            .iter()
            .filter_map(|e| to_coucou(e, &mut seen))
            .map(|v| v["hook_event_name"].as_str().unwrap().to_string())
            .collect()
    }

    #[test]
    fn el_turno_real_no_repite_el_prompt_ni_el_cierre() {
        // Este es el test que faltaba. El anterior montaba un turno ideal a
        // mano (un Thinking, un Done) y por eso pasaba en verde mientras el
        // bridge en produccion mandaba tres UserPromptSubmit y dos Stop por
        // turno.
        //
        // La secuencia real del daemon, medida contra el log de Coucou:
        //   - Thinking llega POR CHUNK (aquí 5 followed de un mismo turno)
        //   - Error cierra el turno y Done llega detrás, porque el forwarder
        //     deja el stream abierto tras un error a propósito (http.rs, "D2
        //     fix") para no perder el ToolResult siguiente
        let s = sid();
        let thinking = |seq: u64| Event::Thinking {
            session_id: s,
            text: "chunk".into(),
            seq,
        };
        let eventos = vec![
            Event::SessionStarted {
                session_id: s,
                agent_id: "default".into(),
            },
            thinking(1),
            thinking(2),
            thinking(3),
            Event::ToolCall {
                session_id: s,
                tool: "list_dir".into(),
                args: json!({}),
                iteration: 1,
                call_id: "c1".into(),
                seq: 4,
            },
            thinking(5),
            Event::Error {
                session_id: Some(s),
                message: "algo".into(),
            },
            Event::Done {
                session_id: s,
                text: String::new(),
            },
        ];

        assert_eq!(
            traza(&eventos),
            vec!["SessionStart", "UserPromptSubmit", "PreToolUse", "Stop"],
            "el prompt y el cierre tienen que salir una vez cada uno"
        );
    }

    #[test]
    fn las_tools_en_paralelo_si_se_reportan_todas() {
        // Contracara del anterior, porque acá la deduplicación NO debe tocar:
        // neurox corre las tools en paralelo (medido: 3 PostToolUse en el
        // mismo instante), y cada uno es trabajo real. Si el set de "vistos"
        // incluyera las tools, la isla mostraría 1 de 3 y el usuario pensaría
        // que el agente solo leyó un archivo.
        let s = sid();
        let call = |i: u32| Event::ToolCall {
            session_id: s,
            tool: format!("tool{i}"),
            args: json!({}),
            iteration: 1,
            call_id: format!("c{i}"),
            seq: 0,
        };
        let res = |i: u32| Event::ToolResult {
            session_id: s,
            tool: format!("tool{i}"),
            result: "ok".into(),
            iteration: 1,
            call_id: format!("c{i}"),
            seq: 0,
        };

        let traza = traza(&[call(1), call(2), call(3), res(1), res(2), res(3)]);
        assert_eq!(
            traza,
            vec![
                "PreToolUse",
                "PreToolUse",
                "PreToolUse",
                "PostToolUse",
                "PostToolUse",
                "PostToolUse"
            ],
            "3 tools en paralelo = 3 reportes de cada clase"
        );
    }

    #[test]
    fn un_turno_nuevo_en_la_misma_sesion_se_vuelve_a_anunciar() {
        // Si `prompted` no se limpiara, el segundo mensaje del usuario caería
        // en silencio y Coucou dejaría de mostrar que neurox está trabajando:
        // seguiría en el estado del turno anterior.
        let s = sid();
        let thinking = Event::Thinking {
            session_id: s,
            text: "a".into(),
            seq: 1,
        };
        let traza = traza(&[
            thinking.clone(),
            Event::Done {
                session_id: s,
                text: String::new(),
            },
            thinking,
            Event::Done {
                session_id: s,
                text: String::new(),
            },
        ]);
        assert_eq!(
            traza,
            vec!["UserPromptSubmit", "Stop", "UserPromptSubmit", "Stop"],
            "cada turno vuelve a anunciarse"
        );
    }

    #[test]
    fn el_cierre_llega_una_vez_aunque_el_error_venga_primero() {
        // El orden real no siempre es Done y después Error. Cualquiera de los
        // dos que llegue primero cierra el turno; el segundo se come.
        let s = sid();
        assert_eq!(
            traza(&[Event::Error {
                session_id: Some(s),
                message: "x".into(),
            }]),
            vec!["Stop"]
        );

        let t = sid();
        assert_eq!(
            traza(&[
                Event::Done {
                    session_id: t,
                    text: String::new(),
                },
                Event::Error {
                    session_id: Some(t),
                    message: "x".into(),
                }
            ]),
            vec!["Stop"]
        );
    }

    #[test]
    fn las_sesiones_no_se_mezclan() {
        // El `Seen` es global al bridge, no por sesión: si el de una sesión
        // contaminara el de otra, una segunda sesión de neurox no se
        // anunciaría nunca.
        let a = sid();
        let b = sid();
        let thinking = |s: Uuid| Event::Thinking {
            session_id: s,
            text: "x".into(),
            seq: 1,
        };
        assert_eq!(
            traza(&[thinking(a), thinking(b)]),
            vec!["UserPromptSubmit", "UserPromptSubmit"],
            "cada sesión anuncia su propio prompt"
        );
    }

    #[test]
    fn el_contenido_por_chunk_no_se_reenvia() {
        // El punto donde esta integracion se podria meter en un circulo vicioso:
        // Content llega dozens de veces por respuesta. Si se reenviara, la isla
        // recibiria una avalanche de eventos que Coucou no sabe pintar.
        let s = sid();
        let eventos: Vec<Event> = (1..50)
            .map(|seq| Event::Content {
                session_id: s,
                text: "hola".into(),
                seq,
            })
            .collect();
        assert!(traza(&eventos).is_empty());
        // Lo mismo con los eventos de metricas y de agente.
        assert!(traza(&[Event::AgentSpawned {
            session_id: s,
            agent_id: "a".into(),
            ephemeral_id: "e".into(),
        }])
        .is_empty());
        assert!(traza(&[Event::Metrics {
            session_id: s,
            iteration: 1,
            tokens_total: 10,
            elapsed_ms: 5,
        }])
        .is_empty());
    }

    #[test]
    fn tool_call_y_tool_result_cargan_el_nombre_de_la_tool() {
        let s = sid();
        let call = to_coucou(
            &Event::ToolCall {
                session_id: s,
                tool: "write_file".into(),
                args: json!({}),
                iteration: 1,
                call_id: "c1".into(),
                seq: 1,
            },
            &mut Seen::default(),
        )
        .unwrap();
        assert_eq!(call["tool_name"], "write_file");
        assert_eq!(call["coucou_agent"], AGENT_NAME);

        let res = to_coucou(
            &Event::ToolResult {
                session_id: s,
                tool: "write_file".into(),
                result: String::new(),
                iteration: 1,
                call_id: "c1".into(),
                seq: 2,
            },
            &mut Seen::default(),
        )
        .unwrap();
        assert_eq!(res["tool_name"], "write_file");
    }

    #[test]
    fn un_error_cierra_la_pill_igual_que_un_stop() {
        // Si se descartara, la pill quedaria en "working" para siempre.
        let s = sid();
        let ev = to_coucou(
            &Event::Error {
                session_id: Some(s),
                message: "boom".into(),
            },
            &mut Seen::default(),
        )
        .unwrap();
        assert_eq!(ev["hook_event_name"], "Stop");

        // Un error sin sesión no se puede atribuir a ninguna pill.
        assert!(to_coucou(
            &Event::Error {
                session_id: None,
                message: "boom".into(),
            },
            &mut Seen::default()
        )
        .is_none());
    }

    #[test]
    fn el_payload_lleva_la_forma_que_coucou_espera() {
        // Coucou lee session_id como string plano y exige hook_event_name.
        let s = sid();
        let v = to_coucou(
            &Event::Thinking {
                session_id: s,
                text: "x".into(),
                seq: 1,
            },
            &mut Seen::default(),
        )
        .unwrap();
        assert!(v["hook_event_name"].is_string());
        assert_eq!(v["session_id"].as_str().unwrap().len(), 36);
        assert_eq!(v["session_id"].as_str().unwrap(), s.to_string());
    }

    #[test]
    fn una_aprobacion_no_pasa_por_el_socket_directo() {
        // El camino de una aprobación es un subproceso, no un write. Si algún
        // día `to_coucou` empieza a mapear `ApprovalRequest`, vuelve el
        // problema que motivó el hook: el hook espera al usuario y el bucle del
        // bus se detiene.
        let req = crate::approval::ApprovalRequest {
            id: uuid::Uuid::new_v4(),
            session_id: sid(),
            tool: "shell".into(),
            args: json!({ "command": "ls" }),
            reason: None,
            created_at: "2026-10-06T00:00:00Z".into(),
        };
        let ev = Event::ApprovalRequest {
            request: req,
        };
        assert!(
            to_coucou(&ev, &mut Seen::default()).is_none(),
            "ApprovalRequest no debe mapearse a un evento directo de Coucou"
        );
    }

    #[test]
    fn el_hook_de_aprobacion_lleva_lo_que_coucou_necesita_para_la_tarjeta() {
        // Coucou arma la tarjeta con `tool_name`, `tool_input` y `prompt`. Si
        // falta alguno, la isla muestra una hoja en blanco y el usuario no sabe
        // qué está autorizando.
        let r = crate::approval::ApprovalRequest {
            id: uuid::Uuid::new_v4(),
            session_id: sid(),
            tool: "write_file".into(),
            args: json!({ "path": "x.rs" }),
            reason: None,
            created_at: "2026-10-06T00:00:00Z".into(),
        };
        let p = serde_json::json!({
            "approval_id": r.id.to_string(),
            "tool_name": r.tool,
            "tool_input": r.args,
            "prompt": format!("Allow {}?", r.tool),
        });
        assert_eq!(p["tool_name"], "write_file");
        assert_eq!(p["tool_input"]["path"], "x.rs");
        assert!(p["prompt"].as_str().unwrap().contains("write_file"));
    }

    #[tokio::test]
    async fn un_socket_inexistente_no_rompe_nada() {
        // El caso normal cuando Coucou no esta instalado: publish devuelve
        // false y no entra en panic.
        let path = PathBuf::from("/run/user/999999/coucou-inexistente.sock");
        let ev = Event::Done {
            session_id: sid(),
            text: "x".into(),
        };
        publish(&ev, &path, &mut Seen::default()).await; // si esto cuelga o peta, el test falla
    }
}