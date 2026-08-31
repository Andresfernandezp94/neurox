//! Skill enable/disable registry (F4.4c + EP-frontend-config).
//!
//! Skills are loaded by `llmd` (the LLM daemon) and surfaced to the
//! daemon via `GET /v1/skills`. The daemon doesn't own the skill
//! content itself — it just tracks the operator's per-skill enable /
//! disable decision so it can:
//!
//! 1. Filter the skill list exposed to the frontend (so the UI can
//!    show toggleable rows with the current state).
//! 2. Forward the decision back to llmd when launching a new chat
//!    (so disabled skills don't end up in the rendered system prompt).
//!
//! EP-0014 C-003: persisted to a JSON sidecar file (path provided at
//! construction). The set survives daemon restarts.

use parking_lot::RwLock;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

/// One row in the operator-facing skill listing.
#[derive(Debug, Clone, Serialize)]
pub struct SkillState {
    pub name: String,
    pub enabled: bool,
}

/// In-memory store of `(skill_name -> enabled)`. Built lazily: any
/// `enable` / `disable` / `set_enabled` call creates the entry if
/// missing. Unknown names default to `enabled = true` in
/// `is_enabled`, which keeps the registry a pure override layer rather
/// than an authoritative catalog (the catalog lives in llmd).
pub struct SkillsRegistry {
    state: RwLock<HashMap<String, bool>>,
    /// EP-0014 C-003: optional persistence path. `None` = in-memory only.
    state_path: Option<PathBuf>,
}

impl SkillsRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::with_state_path(None)
    }

    /// EP-0014 C-003: constructor with optional persistence path.
    /// If `state_path` is `Some(p)`, `set_enabled` persists to `p`
    /// and `load_state` reads from it at startup.
    pub fn with_state_path(state_path: Option<PathBuf>) -> Self {
        Self {
            state: RwLock::new(HashMap::new()),
            state_path,
        }
    }

    /// EP-0014 C-003: load persisted state from disk.
    pub fn load_state(&self) -> std::io::Result<()> {
        if let Some(path) = &self.state_path {
            if path.exists() {
                let content = std::fs::read_to_string(path)?;
                let persisted: HashMap<String, bool> = serde_json::from_str(&content)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                let mut g = self.state.write();
                *g = persisted;
            }
        }
        Ok(())
    }

    /// Snapshot the current state for the frontend listing.
    pub fn list(&self) -> Vec<SkillState> {
        let g = self.state.read();
        let mut out: Vec<SkillState> = g
            .iter()
            .map(|(k, v)| SkillState {
                name: k.clone(),
                enabled: *v,
            })
            .collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }

    /// Merge an external skill catalog into the registry. Skill names
    /// already present keep their stored value; new names are added
    /// with `default_enabled`. Use this after `llmd_client.list_skills`
    /// so the registry knows the names that *exist* (the frontend
    /// would otherwise show only skills an operator has touched).
    pub fn sync_from_names<I, S>(&self, names: I, default_enabled: bool)
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut g = self.state.write();
        for name in names {
            let n = name.as_ref().to_string();
            g.entry(n).or_insert(default_enabled);
        }
    }

    /// Set the enabled flag for a single skill. Creates the entry if
    /// it didn't exist (operator toggled a skill llmd hasn't reported
    /// yet — fine, it'll take effect once llmd loads it).
    pub fn set_enabled(&self, name: &str, enabled: bool) {
        let snapshot = {
            let mut g = self.state.write();
            g.insert(name.to_string(), enabled);
            g.clone()
        };
        // EP-0014 C-003: persist after each change. Best-effort.
        if let Some(path) = &self.state_path {
            if let Ok(content) = serde_json::to_string_pretty(&snapshot) {
                let _ = std::fs::write(path, content);
            }
        }
    }

    /// Equivalent to `set_enabled(name, true)`.
    pub fn enable(&self, name: &str) {
        self.set_enabled(name, true);
    }

    /// Equivalent to `set_enabled(name, false)`.
    pub fn disable(&self, name: &str) {
        self.set_enabled(name, false);
    }

    /// Lookup the stored value. Returns `default_enabled` when the
    /// skill has never been toggled, so callers can treat the registry
    /// as a pure override layer.
    pub fn is_enabled(&self, name: &str, default_enabled: bool) -> bool {
        self.state
            .read()
            .get(name)
            .copied()
            .unwrap_or(default_enabled)
    }
}

impl Default for SkillsRegistry {
    fn default() -> Self {
        Self::new()
    }
}

pub type SharedSkillsRegistry = Arc<SkillsRegistry>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_skill_defaults_to_enabled() {
        let r = SkillsRegistry::new();
        assert!(r.is_enabled("llmd-skill-a", true));
        assert!(!r.is_enabled("llmd-skill-a", false));
    }

    #[test]
    fn set_enabled_persists() {
        let r = SkillsRegistry::new();
        r.disable("skill-x");
        assert!(!r.is_enabled("skill-x", true));
        r.enable("skill-x");
        assert!(r.is_enabled("skill-x", true));
    }

    #[test]
    fn sync_from_names_preserves_existing_state() {
        let r = SkillsRegistry::new();
        r.disable("skill-a");
        r.sync_from_names(["skill-a", "skill-b"], true);
        assert!(!r.is_enabled("skill-a", true));
        assert!(r.is_enabled("skill-b", true));
    }

    #[test]
    fn list_is_sorted_by_name() {
        let r = SkillsRegistry::new();
        r.disable("zeta");
        r.enable("alpha");
        let names: Vec<String> = r.list().into_iter().map(|s| s.name).collect();
        assert_eq!(names, vec!["alpha", "zeta"]);
    }
}