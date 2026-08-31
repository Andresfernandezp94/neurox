use std::path::PathBuf;
use serde_json::Value;
use async_trait::async_trait;
use crate::tools::{Tool, ToolSpec};
use crate::tools::{ToolCategory, Mode};
// GenerateMusicTool extracted from core/src/tools/mod.rs (tools/media/generate_music.rs)
pub struct GenerateMusicTool;

#[async_trait]
impl Tool for GenerateMusicTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "generate_music".to_string(),
            description:
                "Generate music from a text prompt and optional lyrics via MiniMax music-2.6 API. \
                The prompt describes style/mood (e.g. 'Pop, melancholic, rainy night'). \
                Lyrics use structure tags like [Verse], [Chorus], [Bridge]. \
                If no lyrics provided, set is_instrumental or auto-generates lyrics from prompt."
                    .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "prompt": {"type": "string", "description": "Music style/mood description (e.g. 'Blues, Soulful, Rainy Night, Electric Guitar')"},
                    "lyrics": {"type": "string", "description": "Song lyrics with structure tags ([Verse], [Chorus], etc). Optional."},
                    "instrumental": {"type": "boolean", "description": "Generate instrumental only (no vocals). Default false."}
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
            .map_err(|_| "No MINIMAX_API_KEY set. Cannot generate music.".to_string())?;

        let lyrics = args.get("lyrics").and_then(|v| v.as_str()).unwrap_or("");
        let instrumental = args
            .get("instrumental")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let ts = chrono::Utc::now().format("%Y%m%d-%H%M%S");
        let slug: String = prompt
            .chars()
            .take(30)
            .map(|c| {
                if c.is_alphanumeric() {
                    c.to_ascii_lowercase()
                } else {
                    '-'
                }
            })
            .collect();
        // Output dir: $HOME/neurox/music/ (plano, sin subdir de agent).
        // EP-2026-08-19: ruta plana sin acentos (`Música` rompía el
        // URL-encoding del frontend) y sin subdir de agent.
        let music_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("neurox")
            .join("music");
        tokio::fs::create_dir_all(&music_dir)
            .await
            .map_err(|e| format!("mkdir: {e}"))?;
        let output_path = music_dir.join(format!("{}-{}.mp3", slug, ts));

        // Build payload per MiniMax music-2.6 API docs
        let mut payload = serde_json::json!({
            "model": "music-2.6",
            "prompt": prompt,
            "output_format": "url",
            "audio_setting": {
                "sample_rate": 44100,
                "bitrate": 256000,
                "format": "mp3"
            }
        });

        if instrumental {
            payload["is_instrumental"] = Value::Bool(true);
        } else if !lyrics.is_empty() {
            payload["lyrics"] = Value::String(lyrics.to_string());
        } else {
            // No lyrics and not instrumental — auto-generate lyrics from prompt
            payload["lyrics_optimizer"] = Value::Bool(true);
        }

        // Call MiniMax API via reqwest. Music generation can take up to
        // 5 minutes for a full song, so we use a long timeout. No shell,
        // no API key in /proc/PID/cmdline.
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .user_agent("neurox/0.1")
            .build()
            .map_err(|e| format!("client: {e}"))?;

        let resp = client
            .post("https://api.minimax.io/v1/music_generation")
            .bearer_auth(&api_key)
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("music api request: {e}"))?;

        let status = resp.status();
        let body = resp.text().await.map_err(|e| format!("read body: {e}"))?;

        if !status.is_success() {
            return Err(format!(
                "Music API HTTP {}: {}",
                status,
                body.chars().take(300).collect::<String>()
            ));
        }

        let resp_json: Value =
            serde_json::from_str(&body).map_err(|e| format!("parse response: {e}"))?;

        // Check for API error
        let status_code = resp_json
            .get("base_resp")
            .and_then(|b| b.get("status_code"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        if status_code != 0 {
            let msg = resp_json
                .get("base_resp")
                .and_then(|b| b.get("status_msg"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown error");
            return Err(format!("Music API error ({}): {}", status_code, msg));
        }

        // Response format: data.audio contains either a URL (output_format=url) or hex audio
        let audio = resp_json
            .get("data")
            .and_then(|d| d.get("audio"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                format!(
                    "No audio in response: {}",
                    body.chars().take(300).collect::<String>()
                )
            })?
            .to_string();

        // If output_format=url, audio is a URL — download it
        if audio.starts_with("http") {
            let bytes = client
                .get(&audio)
                .send()
                .await
                .map_err(|e| format!("download: {e}"))?
                .bytes()
                .await
                .map_err(|e| format!("download bytes: {e}"))?;
            tokio::fs::write(&output_path, &bytes)
                .await
                .map_err(|e| format!("write file: {e}"))?;
        } else {
            // Hex-encoded audio — decode and write
            let bytes: Result<Vec<u8>, _> = (0..audio.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&audio[i..i + 2], 16))
                .collect();
            let bytes = bytes.map_err(|e| format!("hex decode: {e}"))?;
            tokio::fs::write(&output_path, &bytes)
                .await
                .map_err(|e| format!("write file: {e}"))?;
        }

        let meta = tokio::fs::metadata(&output_path)
            .await
            .map_err(|e| format!("file: {e}"))?;

        // Get duration info if available
        let duration = resp_json
            .get("extra_info")
            .and_then(|e| e.get("music_duration"))
            .and_then(|v| v.as_i64())
            .map(|ms| format!(" ({}s)", ms / 1000))
            .unwrap_or_default();

        Ok(format!(
            "✓ Music generated and saved to {}{} ({}KB)\nPrompt: '{}'\nPlay with: mpv {}",
            output_path.display(),
            duration,
            meta.len() / 1024,
            prompt,
            output_path.display()
        ))
    }
}
