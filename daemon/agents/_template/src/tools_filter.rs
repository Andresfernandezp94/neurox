//! Tool filtering — controls which tools the LLM sees per request.
//!
//! 🟡 STUB. The simplest version is "no filter" (return all tools).
//!    Copy `agents/agent/src/tools_filter.rs` if you want mode-based
//!    filtering or category triggers.
//!
//! ## What this module is for
//!
//! Two strategies:
//!   1. **Allowlist** — explicit list per mode (e.g., Plan mode only
//!      shows read-only tools).
//!   2. **Keyword triggers** — expand the visible tool set based on
//!      what the user is asking for (filesystem tools when the text
//!      mentions "archivo", web tools when it mentions "busca en
//!      internet", etc.).
//!
//! Most real agents combine both: always-on tools + triggered categories.

use crate::llm::LlmClient;

#[allow(dead_code)]
#[derive(Clone)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
}

#[allow(dead_code)]
pub fn filter_tools(all_tools: &[ToolSpec], _user_text: &str, _mode: &crate::mode::Mode) -> Vec<ToolSpec> {
    // 🟡 Default: return all tools. Replace with mode-based logic.
    all_tools.to_vec()
}

// Keep LlmClient referenced so the module compiles even before you
// wire it into `main.rs`.
#[allow(dead_code)]
fn _ensure_llm_used(_c: &LlmClient) {}
