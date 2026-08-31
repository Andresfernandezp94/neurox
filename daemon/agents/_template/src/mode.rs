//! Mode detection — decides how the agent should respond.
//!
//! 🟡 STUB. The simplest possible version is just `Mode::Build` for
//!    everything (full tool access). Copy `agents/agent/src/mode.rs`
//!    if you want keyword-based detection.
//!
//! ## What this module is for
//!
//! Modes control which tools/skills are available and what the LLM
//! is told in the system prompt. Common modes:
//!
//!   - **Chat** — narrow tool surface, conversational style.
//!   - **Plan** — read-only tools, "investigate, don't act".
//!   - **Build** — full tool access, "execute without asking".
//!
//! Detection is usually keyword-based (substring match on user text).
//! The `instruction()` method returns the prompt snippet injected
//! into the system prompt.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // Chat/Plan only used after you wire keyword detection
pub enum Mode {
    Chat,
    Plan,
    Build,
}

impl Mode {
    #[allow(dead_code)]
    pub fn instruction(&self) -> &'static str {
        match self {
            Mode::Plan => "## MODO ACTUAL: PLAN\nSolo investigas, analizas, propones. NO ejecutes cambios.",
            Mode::Build => "## MODO ACTUAL: BUILD\nTienes poder total. Ejecuta, instala, crea sin preguntar en cada paso.",
            Mode::Chat => "",
        }
    }
}

#[allow(dead_code)]
pub fn detect_mode(_user_text: &str) -> Mode {
    // 🟡 Default to Build for the template — full power, no filter.
    //    Replace with keyword detection if you want Plan/Chat modes.
    Mode::Build
}
