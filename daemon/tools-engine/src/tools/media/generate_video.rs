use std::path::PathBuf;
use serde_json::Value;
use async_trait::async_trait;
use crate::tools::media::sanitize_filename;
use crate::tools::url_safety::{is_safe_target, MAX_DOWNLOAD_BYTES};
use crate::tools::{Tool, ToolSpec};
use crate::tools::{ToolCategory, Mode};
// GenerateVideoTool extracted from core/src/tools/mod.rs (tools/media/generate_video.rs)
pub struct GenerateVideoTool;

#[async_trait]
impl Tool for GenerateVideoTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "generate_video".to_string(),
            description: "Generate a short video from a text prompt. Saves to $HOME/neurox/video/."
                .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "prompt": {"type": "string", "description": "Video description prompt"},
                    "duration": {"type": "integer", "description": "Duration in seconds (optional, default 6)"},
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
            .map_err(|_| "No MINIMAX_API_KEY set. Cannot generate video.".to_string())?;

        let filename = args
            .get("filename")
            .and_then(|v| v.as_str())
            .map(|s| sanitize_filename(&s, "mp4"))
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
                format!("{}-{}.mp4", slug, ts)
            });

        // Output dir: $HOME/neurox/video/ (plano, sin subdir de agent).
        // EP-2026-08-19: ruta plana sin acentos (`Vídeos` rompía el
        // URL-encoding del frontend) y sin subdir de agent.
        let output_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("neurox")
            .join("video");
        tokio::fs::create_dir_all(&output_dir)
            .await
            .map_err(|e| format!("mkdir: {e}"))?;

        let output_path = output_dir.join(&filename);

        // Call MiniMax video generation API. Videos can take up to 2-3
        // minutes for a 6-second clip, so we use a long timeout.
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .user_agent("neurox/0.1")
            .build()
            .map_err(|e| format!("client: {e}"))?;

        let payload = serde_json::json!({
            "model": "video-01",
            "prompt": prompt,
            "duration": args.get("duration").and_then(|v| v.as_i64()).unwrap_or(6),
        });

        let resp = client
            .post("https://api.minimax.io/v1/video_generation")
            .bearer_auth(&api_key)
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("video api request: {e}"))?;

        let status = resp.status();
        let body = resp.text().await.map_err(|e| format!("read body: {e}"))?;

        if !status.is_success() {
            return Err(format!(
                "Video API HTTP {}: {}",
                status,
                body.chars().take(300).collect::<String>()
            ));
        }

        let resp_json: Value =
            serde_json::from_str(&body).map_err(|e| format!("parse response: {e}"))?;

        // Video API is async — three steps:
        //   1. POST /v1/video_generation → task_id
        //   2. GET  /v1/query/video_generation?task_id=...  (poll until done)
        //   3. GET  /v1/files/retrieve?file_id=...           → download_url
        let task_id = resp_json
            .get("task_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                format!(
                    "No task_id in response: {}",
                    body.chars().take(300).collect::<String>()
                )
            })?
            .to_string();

        // Poll the task status. Up to 3 minutes total (180s), checking
        // every 5s. Video generation typically takes 30-90s.
        let poll_url = format!(
            "https://api.minimax.io/v1/query/video_generation?task_id={}",
            task_id
        );
        let file_id = {
            let mut attempts = 0;
            let max_attempts = 36; // 36 * 5s = 180s
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                attempts += 1;
                let poll_resp = client
                    .get(&poll_url)
                    .bearer_auth(&api_key)
                    .send()
                    .await
                    .map_err(|e| format!("poll request: {e}"))?;
                let poll_status = poll_resp.status();
                let poll_body = poll_resp
                    .text()
                    .await
                    .map_err(|e| format!("read poll body: {e}"))?;
                if !poll_status.is_success() {
                    return Err(format!(
                        "Video poll HTTP {}: {}",
                        poll_status,
                        poll_body.chars().take(300).collect::<String>()
                    ));
                }
                let poll_json: Value = serde_json::from_str(&poll_body)
                    .map_err(|e| format!("parse poll: {e}"))?;

                let task_status = poll_json
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let base_status = poll_json
                    .get("base_resp")
                    .and_then(|b| b.get("status_code"))
                    .and_then(|v| v.as_i64())
                    .unwrap_or(-1);

                match task_status {
                    "Success" => {
                        // file_id is at the top level
                        match poll_json.get("file_id").and_then(|v| v.as_str()) {
                            Some(fid) => break fid.to_string(),
                            None => {
                                return Err(format!(
                                    "Success status but no file_id: {}",
                                    poll_body.chars().take(300).collect::<String>()
                                ));
                            }
                        }
                    }
                    "Fail" | "Failed" => {
                        let msg = poll_json
                            .get("base_resp")
                            .and_then(|b| b.get("status_msg"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown error");
                        return Err(format!("Video task failed: {msg}"));
                    }
                    _ => {} // "Processing" or anything else — keep polling
                }

                if base_status != 0 && base_status != -1 {
                    return Err(format!(
                        "Video API error (status {}): {}",
                        base_status, task_status
                    ));
                }
                if attempts >= max_attempts {
                    return Err(format!(
                        "Video generation timed out after {}s (task {})",
                        max_attempts * 5,
                        task_id
                    ));
                }
            }
        };

        // Retrieve the file's download URL.
        let files_url = format!(
            "https://api.minimax.io/v1/files/retrieve?file_id={}",
            file_id
        );
        let files_resp = client
            .get(&files_url)
            .bearer_auth(&api_key)
            .send()
            .await
            .map_err(|e| format!("files retrieve: {e}"))?;
        let files_status = files_resp.status();
        let files_body = files_resp
            .text()
            .await
            .map_err(|e| format!("read files body: {e}"))?;
        if !files_status.is_success() {
            return Err(format!(
                "Files API HTTP {}: {}",
                files_status,
                files_body.chars().take(300).collect::<String>()
            ));
        }
        let files_json: Value =
            serde_json::from_str(&files_body).map_err(|e| format!("parse files: {e}"))?;
        let video_url = files_json
            .get("file")
            .and_then(|f| f.get("download_url"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                format!(
                    "No download_url in files response: {}",
                    files_body.chars().take(300).collect::<String>()
                )
            })?
            .to_string();

        // M-SSRF: same as the other generate_* tools — the URL comes
        // from the upstream API and could be a loopback probe.
        is_safe_target(&video_url)?;

        // Download the video with a streaming read + size cap so a
        // malicious server cannot OOM the daemon.
        let mut resp = client
            .get(&video_url)
            .send()
            .await
            .map_err(|e| format!("download: {e}"))?;
        let content_length = resp.content_length().unwrap_or(0);
        if content_length > MAX_DOWNLOAD_BYTES as u64 {
            return Err(format!(
                "video too large: content-length {content_length} > {MAX_DOWNLOAD_BYTES}"
            ));
        }
        let mut bytes = Vec::with_capacity(content_length as usize);
        while let Some(chunk) = resp.chunk().await.map_err(|e| format!("download chunk: {e}"))? {
            if bytes.len() + chunk.len() > MAX_DOWNLOAD_BYTES {
                return Err(format!(
                    "video exceeded {MAX_DOWNLOAD_BYTES} bytes during download"
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
            "✓ Video generated and saved to {} ({}KB)\nPrompt: '{}'\nPlay with: mpv {}",
            output_path.display(),
            meta.len() / 1024,
            prompt,
            output_path.display()
        ))
    }
}
