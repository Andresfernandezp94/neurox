//! Identity loader — system prompt, always-on rules, facts.
//!
//! 🟡 STUB. Copy the implementation from `agents/agent/src/identity.rs`
//!    and adjust the default system prompt for your agent.
//!
//! ## What this module is for
//!
//! An agent's "identity" is the layered system prompt the LLM sees on
//! every request. It typically composes:
//!   1. `system-prompt.md` — base personality / role
//!   2. `_always-on.md`   — house rules (style, format, hard limits)
//!   3. `facts.yaml`      — active facts about the user / project
//!   4. Context summary from the last compaction
//!   5. Active skills (top-N matched against the user text)
//!   6. Mode-specific instruction (Chat / Plan / Build)
//!
//! All of these are loaded from `NEUROX_IDENTITY_DIR` (default
//! `~/.local/share/neurox/identity/`).

use std::path::Path;

/// Agent's identity bundle. Loaded once at startup, held in
/// `ColdState` (see `main.rs`).
pub struct Identity {
    pub system_prompt: String,
    pub always_on: String,
}

/// Load the identity from the given base path. Implementations should
/// fall back to a sensible default if files are missing, so the
/// agent can boot in CI without any user setup.
#[allow(dead_code)]
pub fn load_identity(base_path: &Path) -> Identity {
    let _ = base_path; // suppress unused warning until you wire it up
    Identity {
        system_prompt: "You are agent-template — replace this with your agent's prompt.".into(),
        always_on: String::new(),
    }
}

/// Compose the final system prompt from the layers above. Called on
/// every `process` request.
#[allow(dead_code)]
pub fn build_system_prompt(identity: &Identity) -> String {
    let mut prompt = identity.system_prompt.clone();
    if !identity.always_on.is_empty() {
        prompt.push_str("\n\n");
        prompt.push_str(&identity.always_on);
    }
    prompt
}
