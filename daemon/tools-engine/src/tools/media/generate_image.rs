use std::path::PathBuf;
use serde_json::Value;
use async_trait::async_trait;
use crate::tools::media::sanitize_filename;
use crate::tools::url_safety::{is_safe_target, MAX_DOWNLOAD_BYTES};
use crate::tools::{Tool, ToolSpec};
use crate::tools::{ToolCategory, Mode};
// GenerateImageTool extracted from core/src/tools/mod.rs (tools/media/generate_image.rs)
pub struct GenerateImageTool;

#[async_trait]
impl Tool for GenerateImageTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "generate_image".to_string(),
            description: "Generate an image from a text prompt. Saves to /neurox/images/."
                .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "prompt": {"type": "string", "description": "Image description prompt"},
                    "filename": {"type": "string", "description": "Output filename (without path). Auto-generated if omitted."}
                },
                "required": ["prompt"]
            }),
            requires_approval: false,
        ..Default::default()
        }
    }

    fn categories(&self) -> Vec<ToolCategory> {
        vec![ToolCategory::Media]
    }

    fn mode_compatible(&self) -> Vec<Mode> {
        vec![Mode::Build, Mode::Chat]
    }


    async fn execute(&self, _ctx: &crate::ExecuteContext, args: Value) -> Result<String, String> {
        let prompt = args
            .get("prompt")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing 'prompt'".to_string())?;

        let api_key = std::env::var("MINIMAX_API_KEY")
            .map_err(|_| "No MINIMAX_API_KEY set. Cannot generate images.".to_string())?;

        let filename = args
            .get("filename")
            .and_then(|v| v.as_str())
            .map(|s| sanitize_filename(&s, "jpg"))
            .unwrap_or_else(|| {
                let slug: String = prompt
                    .chars()
                    .take(40)
                    .map(|c| {
                        if c.is_alphanumeric() {
                            c.to_ascii_lowercase()
                        } else {
                            '-'
                        }
                    })
                    .collect();
                let ts = chrono::Utc::now().format("%Y%m%d-%H%M%S");
                format!("{}-{}.jpg", slug, ts)
            });

        // Output dir: $HOME/neurox/images/ (plano, sin subdir de agent).
        // EP-2026-08-19: ruta plana sin acentos (`Imágenes` rompía el
        // URL-encoding del frontend) y sin subdir de agent (`agent`/
        // legacy id dejó de existir).
        let output_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("neurox")
            .join("images");
        tokio::fs::create_dir_all(&output_dir)
            .await
            .map_err(|e| format!("mkdir: {e}"))?;

        let output_path = output_dir.join(&filename);

        // Call MiniMax image generation API via reqwest (no shell, no API
        // key in /proc/PID/cmdline).
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .user_agent("neurox/0.1")
            .build()
            .map_err(|e| format!("client: {e}"))?;

        let payload = serde_json::json!({
            "model": "image-01",
            "prompt": prompt,
            "n": 1
        });

        let resp = client
            .post("https://api.minimax.io/v1/image_generation")
            .bearer_auth(&api_key)
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("image api request: {e}"))?;

        let status = resp.status();
        let body = resp.text().await.map_err(|e| format!("read body: {e}"))?;

        if !status.is_success() {
            return Err(format!(
                "Image API HTTP {}: {}",
                status,
                body.chars().take(200).collect::<String>()
            ));
        }

        let resp_json: Value =
            serde_json::from_str(&body).map_err(|e| format!("parse response: {e}"))?;

        // Extract image URL from response
        let image_url = resp_json
            .get("data")
            .and_then(|d| d.get("image_urls"))
            .and_then(|u| u.as_array())
            .and_then(|a| a.first())
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                format!(
                    "No image URL in response: {}",
                    body.chars().take(200).collect::<String>()
                )
            })?
            .to_string();

        // M-SSRF: the image URL comes from the upstream API. If the
        // API (or a man-in-the-middle on a non-TLS hop) returns a
        // loopback / private / link-local URL, the daemon would
        // fetch from it. Reject those here.
        is_safe_target(&image_url)?;

        // Download the image. Use a streaming download + size cap
        // so a malicious server cannot OOM the daemon.
        let mut resp = client
            .get(&image_url)
            .send()
            .await
            .map_err(|e| format!("download: {e}"))?;
        let content_length = resp.content_length().unwrap_or(0);
        if content_length > MAX_DOWNLOAD_BYTES as u64 {
            return Err(format!(
                "image too large: content-length {content_length} > {MAX_DOWNLOAD_BYTES}"
            ));
        }
        let mut bytes = Vec::with_capacity(content_length as usize);
        while let Some(chunk) = resp.chunk().await.map_err(|e| format!("download chunk: {e}"))? {
            if bytes.len() + chunk.len() > MAX_DOWNLOAD_BYTES {
                return Err(format!(
                    "image exceeded {MAX_DOWNLOAD_BYTES} bytes during download"
                ));
            }
            bytes.extend_from_slice(&chunk);
        }

        tokio::fs::write(&output_path, &bytes)
            .await
            .map_err(|e| format!("write: {e}"))?;

        let meta = tokio::fs::metadata(&output_path)
            .await
            .map_err(|e| format!("file check: {e}"))?;

        Ok(format!(
            "✓ Image generated and saved to {} ({} bytes)\nPrompt: '{}'",
            output_path.display(),
            meta.len(),
            prompt
        ))
    }
}
