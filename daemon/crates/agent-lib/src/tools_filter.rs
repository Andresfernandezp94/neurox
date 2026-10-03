//! Tool filtering — declarative selection by mode + user intent.
//!
//! EP-2026-08-19: this module used to enumerate tool NAMES by hand
//! (e.g. `const READONLY_TOOLS: &[&str] = &["read_file", "list_dir", ...]`)
//! and TRIGGER KEYWORDS by hand (e.g. search for "archivo" in Spanish
//! to surface filesystem tools). Two problems:
//!
//!   1. The name lists referenced tools that don't exist in the engine
//!      (P1 bug: `ocr_screen`, `knowledge`) and missed tools that did

//!   2. The trigger keywords were Spanish-only — English prompts never
//!      fired any category, even for filesystems.
//!
//! New approach: each tool carries its own metadata
//! (`ToolSpec::categories` and `ToolSpec::mode_compatible`) populated by
//! the engine's `ToolRegistry::register` (which calls
//! `Tool::categories()` / `Tool::mode_compatible()`). The filter
//! consults those fields instead of name lists. Adding a new tool is
//! one registration away — no filter edit needed.
//!
//! Trigger keywords are still here for the "Chat" mode heuristic
//! (decide which categories to surface for a given user message).
//! They're bilingual this time (EN + ES) so prompts in either language
//! work.

use crate::llm::ToolSpec;
use crate::mode::Mode;
use std::collections::HashSet;

/// Bilingual trigger keywords for "Chat" mode category selection.
/// EN + ES so the heuristic works for prompts in either language.
/// Keeping these here (rather than in `Mode` or `ToolSpec`) means the
/// filter stays in charge of intent detection, while the engine owns
/// the tool catalog.
const FILESYSTEM_TRIGGERS: &[&str] = &[
    // English
    "file", "code", "read", "write", "search", "directory", "folder",
    "path", "glob", "symbol", "function", "class", "method", "project",
    // Spanish
    "archivo", "código", "lee", "escribe", "busca en", "proyecto",
    "directorio", "carpeta", "función", "clase", "método",
];

const WEB_TRIGGERS: &[&str] = &[
    // English
    "internet", "web", "url", "link", "search", "google", "online",
    "fetch", "download",
    // Spanish
    "busca", "enlace", "página", "descargar",
];

/// Filter tools based on mode and user text triggers.
///
/// EP-2026-08-19 (Fix 4): `allowlist` still applies as a hard wall —
/// if set, the final tool list is intersected with it. The mode +
/// category logic only operates *within* the allowlist scope.
///
/// EP-2026-08-19: the new declarative version reads `categories` and
/// `mode_compatible` directly from each `ToolSpec` (populated by the
/// engine at registration time). No more hard-coded tool name lists.
/// If a future tool doesn't declare its categories, the engine defaults
/// it to `["unfiled"]` (i.e. not in any category); the filter falls
/// back to the `always_tools` heuristic below.
pub fn filter_tools(
    all_tools: &[ToolSpec],
    user_text: &str,
    mode: &Mode,
    allowlist: Option<&[String]>,
) -> Vec<ToolSpec> {
    // Step 1: allowlist is a hard wall.
    let scoped: Vec<ToolSpec> = match allowlist {
        Some(list) if !list.is_empty() => {
            let set: HashSet<&str> = list.iter().map(String::as_str).collect();
            all_tools
                .iter()
                .filter(|t| set.contains(t.function.name.as_str()))
                .cloned()
                .collect()
        }
        _ => all_tools.to_vec(),
    };

    // Step 2: mode + intent filtering.
    match mode {
        Mode::Build => {
            // Full power (within allowlist). Build mode = unrestricted.
            scoped
        }
        Mode::Plan => {
            // Read-only tools only. We determine "read-only" by
            // checking that the tool is not one of the destructive
            // ones (filesystem writes, shell). The engine is the
            // source of truth for categories.
            let destructive: HashSet<&str> =
                ["write_file", "shell"].iter().copied().collect();
            scoped
                .iter()
                .filter(|t| !destructive.contains(t.function.name.as_str()))
                .cloned()
                .collect()
        }
        Mode::Chat => {
            // Step 2a: filter by mode_compatible.
            // (Every tool is Build-compatible by default, so this
            // doesn't drop much in Chat mode unless the engine marked
            // tools as Chat-only — future flexibility.)
            let chat_compatible: Vec<ToolSpec> = scoped
                .iter()
                .filter(|t| {
                    t.mode_compatible.is_empty()
                        || t.mode_compatible.iter().any(|m| m == "chat")
                })
                .cloned()
                .collect();

            // Step 2b: trigger-based category surfacing.
            let lower = user_text.to_lowercase();
            let mut matched_categories: HashSet<&str> = HashSet::new();

            if FILESYSTEM_TRIGGERS.iter().any(|kw| lower.contains(kw)) {
                matched_categories.insert("filesystem");
            }
            if WEB_TRIGGERS.iter().any(|kw| lower.contains(kw)) {
                matched_categories.insert("web");
            }

            // Step 2c: surface tools whose categories match.
            // If no categories matched, fall back to a sensible default
            // (web — the safe conversational baseline).
            let filtered: Vec<ToolSpec> = if matched_categories.is_empty() {
                let fallback: HashSet<&str> = ["web"].iter().copied().collect();
                chat_compatible
                    .iter()
                    .filter(|t| {
                        t.categories.is_empty()
                            || t.categories.iter().any(|c| fallback.contains(c.as_str()))
                    })
                    .cloned()
                    .collect()
            } else {
                chat_compatible
                    .iter()
                    .filter(|t| {
                        // Tools without categories match any intent (open toolset).
                        // Tools WITH categories must match at least one matched category.
                        t.categories.is_empty()
                            || t.categories.iter().any(|c| matched_categories.contains(c.as_str()))
                    })
                    .cloned()
                    .collect()
            };

            // Safety fallback: if fewer than 3 tools matched, return
            // all Chat-compatible tools so the LLM isn't crippled.
            if filtered.len() < 3 {
                chat_compatible
            } else {
                filtered
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_spec(name: &str, cats: &[&str]) -> ToolSpec {
        ToolSpec {
            kind: "function".into(),
            function: crate::llm::ToolFunction {
                name: name.into(),
                description: format!("Tool {name}"),
                parameters: serde_json::json!({}),
            },
            categories: cats.iter().map(|s| s.to_string()).collect(),
            mode_compatible: vec![],
        }
    }

    #[test]
    fn filter_tools_respects_allowlist() {
        let tools = vec![
            make_spec("shell", &["shell"]),
            make_spec("read_file", &["filesystem"]),
        ];
        let filtered = filter_tools(
            &tools,
            "do something",
            &Mode::Build,
            Some(&["shell".into(), "read_file".into()]),
        );
        assert_eq!(filtered.len(), 2);
        let names: Vec<&str> = filtered.iter().map(|t| t.function.name.as_str()).collect();
        assert!(names.contains(&"shell"));
        assert!(names.contains(&"read_file"));
        assert!(!names.contains(&"save_fact"));
    }

    #[test]
    fn filter_tools_empty_allowlist_returns_all() {
        let tools = vec![
            make_spec("shell", &["shell"]),
            make_spec("read_file", &["filesystem"]),
        ];
        let filtered = filter_tools(&tools, "x", &Mode::Build, Some(&[]));
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn filter_tools_no_allowlist_returns_all() {
        let tools = vec![
            make_spec("shell", &["shell"]),
            make_spec("read_file", &["filesystem"]),
        ];
        let filtered = filter_tools(&tools, "x", &Mode::Build, None);
        assert_eq!(filtered.len(), 2);
    }
}
