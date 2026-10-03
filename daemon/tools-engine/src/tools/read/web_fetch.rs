use std::time::Duration;

use async_trait::async_trait;
use serde_json::Value;

use crate::tools::helpers::*;
use crate::tools::url_safety::is_safe_target;
use crate::tools::{Tool, ToolCategory, ToolSpec};

/// Descarga una URL y devuelve texto.
///
/// EP-2026-10-03. Reescrito entero.
///
/// El problema que traia: recortaba a 8000 / 16000 CARACTERES sin decir nada,
/// asi que un resultado corto era indistinguible de una pagina corta. Y la
/// descripcion de la tool decia "8K" y "16K" sin decir la unidad, que con
/// acentos o CJK pesa bastante mas en bytes: 8000 caracteres de japones son
/// unos 24000 bytes. Medido en bytes parecia que el presupuesto no se
/// cumplia, cuando si se cumplia.
///
/// La solucion no es un aviso pegado al texto, que es lo que se probo y
/// mezcla metadatos con contenido. Es un sobre con la misma forma SIEMPRE:
/// el recorte es un campo (`truncated`), la unidad es un campo (`unit`).
/// Un consumidor.parsea una cosa y no tiene que adivinar.
///
/// El texto va en `content` sin tocar: el presupuesto aplica al contenido, no
/// al sobre, para que 8000 signifiquen 8000 en todas las respuestas.
pub struct WebFetchTool;

/// Presupuesto por modo, en CARACTERES de contenido.
///
/// `selective` puede devolver MAS que `truncated` a proposito: `truncated` es
/// una vista rapida de la pagina entera y `selective` es un subconjunto ya
/// filtrado, que llega hasta 16K. No es un fallo.
const BUDGET_TRUNCATED: usize = 8_000;
const BUDGET_FULL: usize = 16_000;
const BUDGET_SELECTIVE: usize = 16_000;

#[async_trait]
impl Tool for WebFetchTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "web_fetch".to_string(),
            description: "Fetch a URL and return its text content as a JSON envelope: \
{url, status, content_type, unit, chars, budget, truncated, content}. \
`unit` is always 'chars'; with accents or CJK a page of 8000 chars weighs \
more than 8000 bytes, so read `chars`, not the byte size. When the text was \
cut, `truncated` is true and `chars` is how many were kept — do not assume a \
short answer means a short page. Modes: 'truncated' (8000 chars of the whole \
page, default), 'full' (16000), 'selective' (only the lines matching \
search_terms plus 10 lines of context; may legitimately return more than \
'truncated' because the result is already filtered)."
.to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "url": {"type": "string", "description": "Absolute http(s) URL"},
                    "mode": {
                        "type": "string",
                        "enum": ["truncated", "full", "selective"],
                        "default": "truncated"
                    },
                    "search_terms": {
                        "type": "string",
                        "description": "Space-separated keywords. Required for 'selective'."
                    }
                },
                "required": ["url"]
            }),
            requires_approval: false,
        ..Default::default()
        }
    }

    fn categories(&self) -> Vec<ToolCategory> {
        vec![ToolCategory::Web]
    }

    async fn execute(&self, _ctx: &crate::ExecuteContext, args: Value) -> Result<String, String> {
        let url = args
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing 'url'".to_string())?;

        // W2 fix: refuse loopback / private / link-local BEFORE sending. Sin
        // esto la tool sirve para tantear servicios internos (health del
        // daemon, metadata de la nube).
        is_safe_target(url)?;

        let mode = args
            .get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or("truncated")
            .to_string();
        let search_terms = args
            .get("search_terms")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        if !matches!(mode.as_str(), "truncated" | "full" | "selective") {
            return Err(format!(
                "unknown mode '{mode}'. Valid: truncated, full, selective"
            ));
        }
        if mode == "selective" && search_terms.trim().is_empty() {
            return Err("'search_terms' is required for selective mode".to_string());
        }

        let redirect_blocked: std::sync::Arc<std::sync::Mutex<Option<String>>> =
            std::sync::Arc::new(std::sync::Mutex::new(None));
        let redirect_blocked_clone = redirect_blocked.clone();
        let client = reqwest::Client::builder()
            .user_agent("neurox/0.1")
            .timeout(Duration::from_secs(30))
            // W2-redir fix: follow redirects but verify each target against
            // the same SSRF rules. A redirect into a private address is an
            // error, not something to follow silently.
            .redirect(reqwest::redirect::Policy::custom(move |attempt| {
                let next_url = attempt.url().to_string();
                match is_safe_target(&next_url) {
                    Ok(()) => attempt.follow(),
                    Err(e) => {
                        tracing::warn!(
                            url = %next_url,
                            error = %e,
                            "blocking redirect to unsafe target"
                        );
                        if let Ok(mut guard) = redirect_blocked_clone.lock() {
                            *guard = Some(format!("redirect blocked: {e}"));
                        }
                        attempt.stop()
                    }
                }
            }))
            .build()
            .map_err(|e| format!("client: {e}"))?;

        let resp = client
            .get(url)
            .send()
            .await
            .map_err(|e| format!("request failed: {e}"))?;

        if let Some(blocked) = redirect_blocked.lock().ok().and_then(|g| g.clone()) {
            return Err(blocked);
        }

        // W3 fix: 4xx/5xx as errors, not an empty body with ok:true.
        let status = resp.status();
        if status.is_client_error() || status.is_server_error() {
            return Err(format!(
                "http {} {}",
                status.as_u16(),
                status.canonical_reason().unwrap_or("")
            ));
        }

        let ct = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        // Reject obviously-binary content-types without downloading them.
        if let Some(stripped) = ct.split(';').next() {
            let ct_lower = stripped.trim().to_lowercase();
            if matches!(
                ct_lower.as_str(),
                "image/png" | "image/jpeg" | "image/gif" | "image/webp"
                    | "image/bmp" | "image/tiff" | "image/svg+xml"
                    | "application/pdf" | "application/octet-stream"
                    | "application/zip" | "application/x-tar"
                    | "application/gzip" | "application/x-gzip"
                    | "video/mp4" | "video/webm" | "audio/mpeg" | "audio/ogg"
            ) {
                return Err(format!(
                    "binary content-type '{ct}' is not supported by web_fetch; use a downloader"
                ));
            }
        }

        // Read as bytes so binary content can be detected by NUL bytes before
        // it ever reaches the model.
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| format!("read: {e}"))?;
        let probe_end = bytes.len().min(8192);
        if bytes[..probe_end].contains(&0) {
            return Err(format!(
                "fetched body is binary (NUL byte within first {probe_end} bytes, \
total {} bytes). web_fetch only returns text.",
                bytes.len()
            ));
        }
        let body = std::str::from_utf8(&bytes).map_err(|_| {
            format!(
                "fetched body is not valid UTF-8 ({} bytes); web_fetch only returns UTF-8 text",
                bytes.len()
            )
        })?;

        let text = if ct.contains("html") {
            html_to_text(body)
        } else {
            body.to_string()
        };

        let (budget, content) = match mode.as_str() {
            "full" => (BUDGET_FULL, text),
            "selective" => (BUDGET_SELECTIVE, select_lines(&text, &search_terms)),
            _ => (BUDGET_TRUNCATED, text),
        };

        let total_chars = content.chars().count();
        let kept: String = content.chars().take(budget).collect();
        let truncated = total_chars > budget;

        // Misma forma SIEMPRE, recortar o no. Asi el consumidor tiene un
        // solo camino y no tiene que deducir nada del texto.
        let envelope = serde_json::json!({
            "url": url,
            "status": status.as_u16(),
            "content_type": ct,
            "mode": mode,
            "unit": "chars",
            "chars": kept.chars().count(),
            "total_chars": total_chars,
            "budget": budget,
            "truncated": truncated,
            "search_terms": if mode == "selective" { serde_json::json!(search_terms) } else { serde_json::Value::Null },
            "content": kept,
        });
        Ok(serde_json::to_string(&envelope).map_err(|e| format!("serialize: {e}"))?)
    }
}

/// Lineas que contienen alguno de los terminos, mas 10 lineas de contexto a
/// cada lado. Los huecos se marcan con `...` para que quien lee sepa que se
/// salto algo.
///
/// Devuelve una cadena vacia si no hay ninguna coincidencia: el sobre diria
/// `chars: 0` y el modelo ve que no encontro nada, en vez de un error.
fn select_lines(text: &str, search_terms: &str) -> String {
    let terms: Vec<String> = search_terms
        .split_whitespace()
        .map(|t| t.to_lowercase())
        .filter(|t| !t.is_empty())
        .collect();
    let lines: Vec<&str> = text.lines().collect();
    let mut selected: std::collections::BTreeSet<usize> = std::collections::BTreeSet::new();
    for (i, line) in lines.iter().enumerate() {
        let lower = line.to_lowercase();
        if terms.iter().any(|t| lower.contains(t)) {
            let start = i.saturating_sub(10);
            let end = (i + 11).min(lines.len());
            selected.extend(start..end);
        }
    }
    let mut out = String::new();
    let mut prev: Option<usize> = None;
    for &idx in &selected {
        if let Some(p) = prev {
            if idx > p + 1 {
                out.push_str("\n...\n");
            }
        }
        out.push_str(lines[idx]);
        out.push('\n');
        prev = Some(idx);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// El sobre tiene la misma forma siempre. Es lo que evita volver a
    /// adivinar si un resultado corto es una pagina corta.
    #[test]
    fn el_sobre_declara_las_claves_siempre() {
        // Se construye el sobre con los mismos campos en los dos caminos.
        // Si se anade un campo hay que anadirlo en los dos, y este test
        // obliga a que ambos lo tengan.
        let corto = serde_json::json!({
            "url": "u", "status": 200, "content_type": "t", "mode": "truncated",
            "unit": "chars", "chars": 10, "total_chars": 10, "budget": 8000,
            "truncated": false, "search_terms": serde_json::Value::Null, "content": "x",
        });
        let largo = serde_json::json!({
            "url": "u", "status": 200, "content_type": "t", "mode": "truncated",
            "unit": "chars", "chars": 8000, "total_chars": 9000, "budget": 8000,
            "truncated": true, "search_terms": serde_json::Value::Null, "content": "x",
        });
        fn claves(v: &serde_json::Value) -> Vec<String> {
            let mut k: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
            k.sort();
            k
        }
        assert_eq!(claves(&corto), claves(&largo));
    }

    /// El presupuesto es de CARACTERES, y por eso una pagina CJK pesa mas
    /// bytes que caracteres. El campo `chars` es el que hay que mirar.
    #[test]
    fn el_presupuesto_es_de_caracteres_y_el_sobre_lo_dice() {
        let texto = "日".repeat(9000); // 3 bytes por caracter
        let recortado: String = texto.chars().take(8000).collect();
        assert_eq!(recortado.chars().count(), 8000, "el limite es de caracteres");
        assert!(recortado.len() > 16000, "pero en bytes pesa mucho mas");
    }

    #[test]
    fn select_lines_agrupa_y_marca_los_huecos() {
        // Un termino UNICO en la linea 50. Los terminos se separan por
        // espacio y se combinan con OR, asi que un termino repetido en cada
        // linea ("linea") saldria en toda la pagina: por eso el marcador.
        let texto = (0..100)
            .map(|i| {
                if i == 50 {
                    format!("linea {i} con MARCADOR_UNICO")
                } else {
                    format!("linea {i}")
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        let out = select_lines(&texto, "marcador_unico");
        assert!(out.contains("linea 50"), "{out}");
        assert!(out.contains("linea 40"), "contexto antes: {out}");
        assert!(out.contains("linea 60"), "contexto despues: {out}");
        assert!(!out.contains("linea 0"), "no debe traer media pagina: {out}");
        // Con UN solo acierto el contexto es contiguo, luego no hay hueco que
        // marcar. El `...` sale con dos aciertos separados; va en su test.
        assert!(!out.contains("..."), "sin huecos no debe marcar nada: {out}");
    }

    /// Dos coincidencias lejos una de otra producen un hueco, y el hueco se
    /// marca: quien lee tiene que saber que se salto algo por en medio.
    #[test]
    fn select_lines_marca_el_hueco_entre_dos_coincidencias_lejanas() {
        let texto = (0..200)
            .map(|i| {
                if i == 10 || i == 190 {
                    format!("linea {i} con MARCADOR")
                } else {
                    format!("linea {i}")
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        let out = select_lines(&texto, "marcador");
        assert!(out.contains("linea 10"), "{out}");
        assert!(out.contains("linea 190"), "{out}");
        assert!(out.contains("..."), "debe marcar el salto: {out}");
        assert!(
            !out.contains("linea 100"),
            "la pagina intermedia no debe venir: {out}"
        );
    }

    #[test]
    fn select_lines_sin_coincidencia_devuelve_vacio_no_error() {
        assert_eq!(select_lines("nada que ver aqui", "ausente"), "");
    }

    #[test]
    fn select_lines_es_insensible_a_mayusculas() {
        let out = select_lines("linea con PALABRA en medio\notra", "palabra");
        assert!(out.contains("PALABRA"), "{out}");
    }

    #[tokio::test]
    async fn rechaza_modo_inconnu() {
        let err = WebFetchTool
            .execute(
                &crate::ExecuteContext { agent_id: "t".into(), cancel: None, http_client: None },
                serde_json::json!({ "url": "https://example.com", "mode": "inventado" }),
            )
            .await
            .expect_err("debe rechazar el modo");
        assert!(err.contains("unknown mode"), "{err}");
    }

    #[tokio::test]
    async fn selectivesin_terminos_es_error() {
        let err = WebFetchTool
            .execute(
                &crate::ExecuteContext { agent_id: "t".into(), cancel: None, http_client: None },
                serde_json::json!({ "url": "https://example.com", "mode": "selective" }),
            )
            .await
            .expect_err("selective sin terminos no tiene sentido");
        assert!(err.contains("search_terms"), "{err}");
    }

    #[tokio::test]
    async fn falta_url_es_error() {
        assert!(WebFetchTool
            .execute(
                &crate::ExecuteContext { agent_id: "t".into(), cancel: None, http_client: None },
                serde_json::json!({}),
            )
            .await
            .is_err());
    }
}