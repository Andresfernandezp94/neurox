use std::time::Duration;
use serde_json::Value;
use async_trait::async_trait;
use crate::tools::{Tool, ToolSpec};
use crate::tools::helpers::*;
use crate::tools::{ToolCategory, Mode};
// WebFetchTool extracted from core/src/tools/mod.rs (tools/read/web_fetch.rs)
pub struct WebFetchTool;

/// W-fix: block SSRF to loopback / private networks. Resolves the
/// URL's host and rejects any IP that is in a private, loopback, or
/// link-local range. Without this, the LLM could probe internal
/// services (daemon health, cloud metadata, etc.) through web_fetch.
fn is_safe_target(url: &str) -> Result<(), String> {
    let parsed = url::Url::parse(url).map_err(|e| format!("invalid url: {e}"))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(format!("scheme not allowed: {}", parsed.scheme()));
    }
    let host = parsed.host_str().ok_or("no host")?;
    // Resolve the host to one or more IPs. If any IP is unsafe, deny.
    // (We use std::net::ToSocketAddrs via tokio's spawn_blocking to
    // keep the async runtime responsive.)
    let host_owned = host.to_string();
    let port = parsed.port_or_known_default().unwrap_or(80);
    let addrs: Vec<std::net::SocketAddr> = std::net::ToSocketAddrs::to_socket_addrs(
        &format!("{host_owned}:{port}"),
    ).map_err(|e| format!("dns: {e}"))?.collect();
    if addrs.is_empty() {
        return Err("no addresses resolved".into());
    }
    for addr in &addrs {
        let ip = addr.ip();
        if is_unsafe_ip(&ip) {
            return Err(format!(
                "refusing to fetch private/loopback address: {ip} (resolved from {host})"
            ));
        }
    }
    Ok(())
}

fn is_unsafe_ip(ip: &std::net::IpAddr) -> bool {
    use std::net::IpAddr::*;
    match ip {
        V4(v4) => {
            v4.is_loopback()           // 127.0.0.0/8
            || v4.is_private()        // 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16
            || v4.is_link_local()      // 169.254.0.0/16 (cloud metadata!)
            || v4.is_unspecified()     // 0.0.0.0
            || v4.is_broadcast()       // 255.255.255.255
        }
        V6(v6) => {
            v6.is_loopback()           // ::1
            || v6.is_unspecified()    // ::
            // Unique local addresses fc00::/7
            || (v6.segments()[0] & 0xfe00) == 0xfc00
            // Link-local fe80::/10
            || (v6.segments()[0] & 0xffc0) == 0xfe80
        }
    }
}

#[async_trait]
impl Tool for WebFetchTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "web_fetch".to_string(),
            description: "Fetch a URL and return text content. Modes: 'truncated' (8K, default), 'full' (16K), 'selective' (search for keywords, ±10 lines context).".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "url": {"type": "string", "description": "HTTP or HTTPS URL"},
                    "mode": {"type": "string", "enum": ["truncated", "full", "selective"], "default": "truncated"},
                    "search_terms": {"type": "string", "description": "Space-separated keywords for selective mode"}
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

    fn mode_compatible(&self) -> Vec<Mode> {
        vec![Mode::Build, Mode::Plan, Mode::Chat]
    }


    async fn execute(&self, _ctx: &crate::ExecuteContext, args: Value) -> Result<String, String> {
        let url = args
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing 'url'".to_string())?;
        // W2 fix: refuse to fetch loopback / private / link-local URLs
        // BEFORE sending the request. Otherwise the LLM could probe
        // internal services (daemon health, cloud metadata) through
        // web_fetch.
        is_safe_target(url)?;
        let mode = args
            .get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or("truncated");
        let search_terms = args
            .get("search_terms")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let redirect_blocked: std::sync::Arc<std::sync::Mutex<Option<String>>> =
            std::sync::Arc::new(std::sync::Mutex::new(None));
        let redirect_blocked_clone = redirect_blocked.clone();
        let client = reqwest::Client::builder()
            .user_agent("neurox/0.1")
            .timeout(Duration::from_secs(30))
            // W2-redir fix: follow redirects but verify each redirect
            // target against the same SSRF rules. If a redirect
            // would point at a private/loopback address, abort with
            // an error instead of silently following it.
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
                        *redirect_blocked_clone.lock().unwrap() =
                            Some(format!("redirect blocked: {e}"));
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
            .map_err(|e| format!("http: {e}"))?;
        if let Some(err) = redirect_blocked.lock().unwrap().take() {
            return Err(err);
        }
        // W3 fix: surface 4xx/5xx as errors instead of returning an
        // empty body with ok:true.
        let status = resp.status();
        if status.is_client_error() || status.is_server_error() {
            return Err(format!("http {} {}", status.as_u16(), status.canonical_reason().unwrap_or("")));
        }
        let ct = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        // Reject obviously-binary content-types (without downloading).
        if let Some(stripped) = ct.split(';').next() {
            let ct_lower = stripped.trim().to_lowercase();
            if matches!(ct_lower.as_str(),
                "image/png" | "image/jpeg" | "image/gif" | "image/webp"
                | "image/bmp" | "image/tiff" | "image/svg+xml"
                | "application/pdf" | "application/octet-stream"
                | "application/zip" | "application/x-tar"
                | "application/gzip" | "application/x-gzip"
                | "video/mp4" | "video/webm" | "audio/mpeg" | "audio/ogg"
            ) {
                return Err(format!(
                    "binary content-type '{}' is not supported by web_fetch; use a downloader",
                    ct
                ));
            }
        }
        // Read as bytes so we can detect NUL bytes that would
        // otherwise end up in the LLM's response.
        let bytes = resp.bytes().await.map_err(|e| format!("read: {e}"))?;
        // W1 fix: detect binary content by NUL-byte presence (POSIX
        // definition of text vs binary). If NUL byte found within the
        // first 8KB, refuse to return raw bytes — return a clear
        // message instead.
        let probe_end = bytes.len().min(8192);
        if bytes[..probe_end].contains(&0) {
            return Err(format!(
                "fetched body is binary (NUL byte found within first {probe_end} bytes, total {} bytes). web_fetch only returns text.",
                bytes.len()
            ));
        }
        let body = match std::str::from_utf8(&bytes) {
            Ok(s) => s.to_string(),
            Err(_) => return Err(format!(
                "fetched body is not valid UTF-8 ({} bytes); web_fetch only returns UTF-8 text",
                bytes.len()
            )),
        };
        let text = if ct.contains("html") {
            html_to_text(&body)
        } else {
            body
        };

        match mode {
            "full" => {
                let truncated: String = text.chars().take(16000).collect();
                Ok(truncated)
            }
            "selective" => {
                if search_terms.is_empty() {
                    return Err("'search_terms' required for selective mode".to_string());
                }
                let terms: Vec<&str> = search_terms.split_whitespace().collect();
                let lines: Vec<&str> = text.lines().collect();
                let mut selected_lines: std::collections::BTreeSet<usize> =
                    std::collections::BTreeSet::new();

                for (i, line) in lines.iter().enumerate() {
                    let lower = line.to_lowercase();
                    if terms.iter().any(|t| lower.contains(&t.to_lowercase())) {
                        let start = i.saturating_sub(10);
                        let end = (i + 11).min(lines.len());
                        for idx in start..end {
                            selected_lines.insert(idx);
                        }
                    }
                }

                if selected_lines.is_empty() {
                    return Ok(format!(
                        "No matches for '{}' in page content.",
                        search_terms
                    ));
                }

                let mut out = String::new();
                let mut prev: Option<usize> = None;
                for &idx in &selected_lines {
                    if let Some(p) = prev {
                        if idx > p + 1 {
                            out.push_str("\n...\n\n");
                        }
                    }
                    out.push_str(lines[idx]);
                    out.push('\n');
                    prev = Some(idx);
                }
                // Truncate to 16K
                let result: String = out.chars().take(16000).collect();
                Ok(result)
            }
            _ => {
                // truncated (default): 8K
                let truncated: String = text.chars().take(8000).collect();
                Ok(truncated)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // W1: is_safe_target blocks loopback, link-local, private, etc.
    #[test]
    fn safe_target_blocks_loopback() {
        assert!(is_safe_target("http://127.0.0.1/").is_err());
        assert!(is_safe_target("http://127.0.0.1:7878/health").is_err());
        assert!(is_safe_target("http://localhost/foo").is_err()); // resolves to 127.0.0.1
    }

    #[test]
    fn safe_target_blocks_private() {
        assert!(is_safe_target("http://10.0.0.1/").is_err());
        assert!(is_safe_target("http://192.168.1.1/").is_err());
        assert!(is_safe_target("http://172.16.0.1/").is_err());
        assert!(is_safe_target("http://169.254.169.254/latest/").is_err()); // cloud metadata
    }

    #[test]
    fn safe_target_blocks_non_http() {
        assert!(is_safe_target("file:///etc/passwd").is_err());
        assert!(is_safe_target("javascript:alert(1)").is_err());
        assert!(is_safe_target("gopher://example.com").is_err());
    }

    #[test]
    fn safe_target_allows_public() {
        assert!(is_safe_target("https://example.com").is_ok());
        assert!(is_safe_target("https://httpbin.org/get").is_ok());
    }

    #[test]
    fn safe_target_rejects_invalid_url() {
        assert!(is_safe_target("not-a-url").is_err());
    }

    // is_unsafe_ip — covers IPv4 and IPv6 ranges.
    #[test]
    fn unsafe_ip_categorization() {
        use std::net::IpAddr;
        // Loopback
        assert!(is_unsafe_ip(&"127.0.0.1".parse::<IpAddr>().unwrap()));
        assert!(is_unsafe_ip(&"::1".parse::<IpAddr>().unwrap()));
        // Private
        assert!(is_unsafe_ip(&"10.1.2.3".parse::<IpAddr>().unwrap()));
        assert!(is_unsafe_ip(&"192.168.1.1".parse::<IpAddr>().unwrap()));
        // Link-local (cloud metadata)
        assert!(is_unsafe_ip(&"169.254.169.254".parse::<IpAddr>().unwrap()));
        // Public
        assert!(!is_unsafe_ip(&"8.8.8.8".parse::<IpAddr>().unwrap()));
        assert!(!is_unsafe_ip(&"1.1.1.1".parse::<IpAddr>().unwrap()));
    }
}
