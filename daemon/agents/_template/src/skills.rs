//! Skills — domain-specific instructions injected on demand.
//!
//! 🟡 STUB. Copy the implementation from `agents/agent/src/skills.rs`.
//!
//! ## What this module is for
//!
//! A "skill" is a named bundle of instructions the LLM receives when
//! the user's text matches the skill's trigger keywords. Examples:
//!   - "code-review" — when the user says "review this PR"
//!   - "data-analysis" — when the user mentions a CSV or dataset
//!   - "writing-style-newsroom" — for journalistic tone
//!
//! Skills are loaded from `NEUROX_IDENTITY_DIR/skills/*.md` (frontmatter
//! for triggers + name, body for instructions).

#[allow(dead_code)]
pub struct Skill {
    pub name: String,
    pub instructions: String,
    #[allow(dead_code)]
    pub preferred_tools: Vec<String>,
}

#[allow(dead_code)]
pub fn load_skills(_skills_dir: &std::path::Path) -> Vec<Skill> {
    Vec::new()
}

#[allow(dead_code)]
pub fn match_skills(_skills: &[Skill], _user_text: &str, _top_n: usize) -> Vec<Skill> {
    Vec::new()
}
