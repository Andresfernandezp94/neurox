//! Identity loader — reads the agent manifest (e.g. `Default.json`) and
//! exposes its fields. The manifest is the single source of truth for
//! file locations, LLM sampling, tool policy, memory tiers, budget,
//! stop conditions, permissions, and observability.
//!
//! EP-2026-08-19 (v2 manifest): the `files.harness` field is now an
//! array of paths (concatenated in order). The old `files.always_on`
//! + `files.facts` are removed; use `files.harness[]` and
//! `memory.semantic.path` instead.
//!
//! If a manifest is not found at `<identity_dir>/<agent_id>.json`, we
//! fall back to historical defaults so older agents still work.

use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// ─────────────────────────────────────────────────────────────────
// v2 manifest types
// ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Meta {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
    #[serde(default)]
    pub extends: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Files {
    #[serde(default = "default_prompt_path")]
    pub prompt: String,
    #[serde(default)]
    pub harness: Vec<String>,
    #[serde(default = "default_skills_dir_path")]
    pub skills_dir: String,
}

fn default_prompt_path() -> String {
    "prompt.md".to_string()
}
fn default_skills_dir_path() -> String {
    "skills/".to_string()
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Sampling {
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub top_p: Option<f32>,
    #[serde(default)]
    pub top_k: Option<i32>,
    #[serde(default)]
    pub max_tokens: Option<i32>,
    #[serde(default)]
    pub stop_sequences: Vec<String>,
    #[serde(default)]
    pub seed: Option<i64>,
    #[serde(default)]
    pub frequency_penalty: Option<f32>,
    #[serde(default)]
    pub presence_penalty: Option<f32>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct LlmConfig {
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub sampling: Sampling,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ToolPolicy {
    /// "allow_all" | "allowlist" | "denylist"
    #[serde(default = "default_policy_type")]
    pub r#type: String,
    #[serde(default)]
    pub allowlist: Option<Vec<String>>,
    #[serde(default)]
    pub denylist: Option<Vec<String>>,
}

fn default_policy_type() -> String {
    "allow_all".to_string()
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ToolsConfig {
    #[serde(default)]
    pub policy: ToolPolicy,
    /// Per-tool config. Free-form — each tool can have its own keys
    /// (e.g. `generate_video.default_duration_secs`).
    #[serde(default)]
    pub config: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct McpServer {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub transport: String,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub auth: Option<McpAuth>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct McpAuth {
    #[serde(default)]
    pub r#type: String,
    #[serde(default)]
    pub token_env: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct MemoryTier {
    #[serde(default)]
    pub r#type: String,
    #[serde(default)]
    pub max_messages: Option<i32>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub embedding_model: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SemanticMemory {
    #[serde(default)]
    pub r#type: String,
    #[serde(default)]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct MemoryConfig {
    #[serde(default)]
    pub short_term: MemoryTier,
    #[serde(default)]
    pub long_term: MemoryTier,
    #[serde(default)]
    pub episodic: MemoryTier,
    #[serde(default)]
    pub semantic: SemanticMemory,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct GuardrailsConfig {
    #[serde(default)]
    pub input: Vec<serde_json::Value>,
    #[serde(default)]
    pub output: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct OutputConfig {
    #[serde(default = "default_output_type")]
    pub r#type: String,
    #[serde(default)]
    pub schema: Option<serde_json::Value>,
}

fn default_output_type() -> String {
    "text".to_string()
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct StreamingConfig {
    #[serde(default = "default_streaming_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub chunk_size: Option<i32>,
}

fn default_streaming_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct BudgetConfig {
    #[serde(default)]
    pub max_tokens_per_run: Option<i64>,
    #[serde(default)]
    pub max_runtime_secs: Option<i64>,
    #[serde(default)]
    pub cost_limit_usd: Option<f32>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct StopConditionsConfig {
    #[serde(default = "default_max_iterations")]
    pub max_iterations: i32,
    #[serde(default)]
    pub max_tool_calls: Option<i32>,
    #[serde(default)]
    pub max_duration_secs: Option<i64>,
}

fn default_max_iterations() -> i32 {
    10
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SubAgent {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub instructions: String,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub model_override: Option<String>,
    #[serde(default)]
    pub handoff_description: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SandboxConfig {
    #[serde(default)]
    pub writable_paths: Vec<String>,
    #[serde(default)]
    pub readable_paths: Vec<String>,
    #[serde(default = "default_sandbox_flag")]
    pub network_access: bool,
    #[serde(default = "default_sandbox_flag")]
    pub process_spawn: bool,
    #[serde(default)]
    pub memory_limit_mb: Option<i32>,
    #[serde(default)]
    pub cpu_limit_pct: Option<i32>,
}

fn default_sandbox_flag() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct PermissionsConfig {
    #[serde(default)]
    pub sandbox: SandboxConfig,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ObservabilityConfig {
    #[serde(default = "default_log_level")]
    pub log_level: String,
    #[serde(default)]
    pub trace: bool,
    #[serde(default)]
    pub metrics: bool,
}

fn default_log_level() -> String {
    "info".to_string()
}

/// Full v2 manifest. Fields not used by the agent binary yet are
/// still parsed (and logged) so the structure is observable.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct Manifest {
    #[serde(default)]
    pub meta: Meta,
    #[serde(default)]
    pub files: Files,
    #[serde(default)]
    pub llm: LlmConfig,
    #[serde(default)]
    pub tools: ToolsConfig,
    #[serde(default)]
    pub mcp_servers: Vec<McpServer>,
    #[serde(default)]
    pub memory: MemoryConfig,
    #[serde(default)]
    pub guardrails: GuardrailsConfig,
    #[serde(default)]
    pub output: OutputConfig,
    #[serde(default)]
    pub streaming: StreamingConfig,
    #[serde(default)]
    pub budget: BudgetConfig,
    #[serde(default)]
    pub stop_conditions: StopConditionsConfig,
    #[serde(default)]
    pub sub_agents: Vec<SubAgent>,
    #[serde(default)]
    pub permissions: PermissionsConfig,
    #[serde(default)]
    pub observability: ObservabilityConfig,
}

// ─────────────────────────────────────────────────────────────────
// Identity (kept for backwards compat with current build_system_prompt)
// ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Identity {
    pub system_prompt: String,
    /// Concatenated contents of all `files.harness[]` files.
    pub harness: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Fact {
    #[allow(dead_code)]
    pub id: String,
    #[serde(rename = "type")]
    #[allow(dead_code)]
    pub kind: String,
    pub content: String,
    #[serde(default)]
    pub active: bool,
}

#[derive(Debug, Deserialize)]
struct FactsFile {
    facts: Vec<Fact>,
}

pub struct ActiveSkill {
    pub name: String,
    pub instructions: String,
}

// ─────────────────────────────────────────────────────────────────
// Loaders
// ─────────────────────────────────────────────────────────────────

/// Load the v2 manifest from `<base_path>/<agent_id>.json`.
/// Returns `None` if the file doesn't exist or fails to parse — caller
/// falls back to hardcoded defaults.
pub fn load_manifest(base_path: &Path, agent_id: &str) -> Option<Manifest> {
    let path = base_path.join(format!("{}.json", agent_id));
    match std::fs::read_to_string(&path) {
        Ok(content) => match serde_json::from_str::<Manifest>(&content) {
            Ok(m) => {
                eprintln!(
                    "[default] loaded manifest agent={} schema=v2 harness={} tools.policy={} stop.max_iter={} obs.log_level={}",
                    agent_id,
                    m.files.harness.len(),
                    m.tools.policy.r#type,
                    m.stop_conditions.max_iterations,
                    m.observability.log_level,
                );
                Some(m)
            }
            Err(e) => {
                eprintln!("[default] WARN: failed to parse manifest {}: {e}", path.display());
                None
            }
        },
        Err(_) => None,
    }
}

/// Read the system prompt + concatenated harness content. Paths come
/// from the manifest (or hardcoded defaults if no manifest).
pub fn load_identity(base_path: &Path, manifest: Option<&Manifest>) -> Identity {
    let prompt_rel = manifest
        .map(|m| m.files.prompt.as_str())
        .unwrap_or("prompt.md");
    let harness_rels: Vec<&str> = manifest
        .map(|m| m.files.harness.iter().map(String::as_str).collect())
        .unwrap_or_default();

    let system_prompt = std::fs::read_to_string(base_path.join(prompt_rel)).unwrap_or_else(|e| {
        eprintln!("[default] WARN: could not read {}: {e}", prompt_rel);
        "You are default, a helpful agent.".to_string()
    });

    let mut harness = String::new();
    for rel in &harness_rels {
        match std::fs::read_to_string(base_path.join(rel)) {
            Ok(content) => {
                if !harness.is_empty() {
                    harness.push_str("\n\n");
                }
                harness.push_str(&content);
            }
            Err(e) => {
                eprintln!("[default] WARN: could not read harness {}: {e}", rel);
            }
        }
    }

    Identity {
        system_prompt,
        harness,
    }
}

/// Read facts from `memory.semantic.path` (or `facts.yaml` as
/// historical fallback). The path is taken from the manifest.
pub fn load_facts(base_path: &Path, manifest: Option<&Manifest>) -> Vec<Fact> {
    let facts_rel = manifest
        .and_then(|m| m.memory.semantic.path.as_deref())
        .unwrap_or("facts.yaml");
    let path = base_path.join(facts_rel);
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[default] WARN: could not read {}: {e}", facts_rel);
            return Vec::new();
        }
    };
    match serde_yml::from_str::<FactsFile>(&content) {
        Ok(f) => f.facts,
        Err(e) => {
            eprintln!("[default] WARN: failed to parse {}: {e}", facts_rel);
            Vec::new()
        }
    }
}

/// Resolve the skills directory from the manifest (or default
/// `skills/`).
pub fn skills_dir(base_path: &Path, manifest: Option<&Manifest>) -> PathBuf {
    let rel = manifest
        .map(|m| m.files.skills_dir.as_str())
        .unwrap_or("skills/");
    base_path.join(rel)
}

/// EP-2026-08-19: load long-term memory from `memory.long_term.path`.
/// Returns empty Vec if the file doesn't exist or fails to parse. Each
/// line is a JSON object representing a long-term memory entry.
pub fn load_long_term_memory(base_path: &Path, manifest: Option<&Manifest>) -> Vec<serde_json::Value> {
    let Some(path) = manifest.and_then(|m| m.memory.long_term.path.as_ref()) else {
        return Vec::new();
    };
    let full = base_path.join(path);
    let content = match std::fs::read_to_string(&full) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

/// EP-2026-08-19: load episodic memory from `memory.episodic.path`.
/// Returns empty Vec if the file doesn't exist. Each line is JSON
/// representing an episode.
pub fn load_episodic_memory(base_path: &Path, manifest: Option<&Manifest>) -> Vec<serde_json::Value> {
    let Some(path) = manifest.and_then(|m| m.memory.episodic.path.as_ref()) else {
        return Vec::new();
    };
    let full = base_path.join(path);
    let content = match std::fs::read_to_string(&full) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

/// Build the full system prompt combining identity, harness, facts,
/// context summary, active skills, and mode.
pub fn build_system_prompt(
    identity: &Identity,
    facts: &[Fact],
    context_summary: &str,
    active_skills: &[ActiveSkill],
    mode_instruction: &str,
) -> String {
    let mut prompt = identity.system_prompt.clone();

    // Harness (concatenated rules + context)
    if !identity.harness.is_empty() {
        prompt.push_str("\n\n");
        prompt.push_str(&identity.harness);
    }

    // Active facts
    let active_facts: Vec<&Fact> = facts.iter().filter(|f| f.active).collect();
    if !active_facts.is_empty() {
        prompt.push_str("\n\n## Memoria activa (facts)\n");
        for fact in &active_facts {
            prompt.push_str(&format!("- {}\n", fact.content));
        }
    }

    if !context_summary.is_empty() {
        prompt.push_str("\n\n## Resumen de contexto previo\n");
        prompt.push_str(context_summary);
        prompt.push('\n');
    }

    if !active_skills.is_empty() {
        prompt.push_str("\n\n## Skills activas\n");
        for skill in active_skills {
            prompt.push_str(&format!("### {}\n{}\n\n", skill.name, skill.instructions));
        }
    }

    if !mode_instruction.is_empty() {
        prompt.push_str("\n\n");
        prompt.push_str(mode_instruction);
        prompt.push('\n');
    }

    prompt
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_temp_identity(prompt: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("prompt.md"), prompt).unwrap();
        dir
    }

    #[test]
    fn load_identity_missing_prompt_returns_fallback() {
        let dir = tempfile::tempdir().unwrap();
        let id = load_identity(dir.path(), None);
        // Fallback string used when prompt.md is missing.
        assert!(id.system_prompt.contains("default"));
    }

    #[test]
    fn load_identity_reads_prompt() {
        let dir = make_temp_identity("You are a test agent.");
        let id = load_identity(dir.path(), None);
        assert_eq!(id.system_prompt, "You are a test agent.");
    }

    #[test]
    fn load_identity_concatenates_harness() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("prompt.md"), "BASE").unwrap();
        std::fs::write(dir.path().join("h1.md"), "HARNESS1").unwrap();
        let manifest = Manifest {
            files: crate::identity::Files {
                prompt: "prompt.md".into(),
                harness: vec!["h1.md".into()],
                ..Default::default()
            },
            ..Default::default()
        };
        let id = load_identity(dir.path(), Some(&manifest));
        assert!(id.system_prompt.contains("BASE"));
        assert!(id.harness.contains("HARNESS1"));
    }
}
