use serde_json::Value;
use async_trait::async_trait;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use crate::tools::{Tool, ToolSpec};
use crate::tools::{ToolCategory, Mode};
// ClipboardWriteTool extracted from core/src/tools/mod.rs (tools/desktop/clipboard_write.rs)
pub struct ClipboardWriteTool;

#[async_trait]
impl Tool for ClipboardWriteTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "clipboard_write".to_string(),
            description: "Write text to the clipboard (Wayland wl-copy).".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string", "description": "Text to copy to clipboard"}
                },
                "required": ["text"]
            }),
            requires_approval: true,
        ..Default::default()
        }
    }

    fn categories(&self) -> Vec<ToolCategory> {
        vec![ToolCategory::Desktop]
    }

    fn mode_compatible(&self) -> Vec<Mode> {
        vec![Mode::Build, Mode::Chat]
    }


    async fn execute(&self, _ctx: &crate::ExecuteContext, args: Value) -> Result<String, String> {
        let text = args
            .get("text")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing 'text'".to_string())?;

        // CW-fix: cap the clipboard payload. The LLM can ship a
        // multi-MB string here and (a) fill the wl-copy pipe buffer
        // and break the connection or (b) overwrite the user's actual
        // clipboard with junk. 1 MiB is plenty for a real clipboard
        // payload (the OS clipboard typically holds 1-100 KB).
        const MAX_CLIPBOARD_BYTES: usize = 1024 * 1024;
        if text.len() > MAX_CLIPBOARD_BYTES {
            return Err(format!(
                "clipboard_write: text too long ({} bytes, max {})",
                text.len(),
                MAX_CLIPBOARD_BYTES
            ));
        }

        let mut child = Command::new("wl-copy")
            .stdin(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("wl-copy spawn failed: {e}"))?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(text.as_bytes())
                .await
                .map_err(|e| format!("write to wl-copy: {e}"))?;
        }

        let status = child
            .wait()
            .await
            .map_err(|e| format!("wl-copy wait: {e}"))?;
        if !status.success() {
            return Err("wl-copy exited with error".to_string());
        }

        Ok(format!("copied {} chars to clipboard", text.len()))
    }
}
