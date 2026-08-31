//! Skill loader — reads .md skill files and matches by keyword.

use regex::Regex;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Skill {
    pub name: String,
    pub trigger_keywords: Vec<String>,
    pub instructions: String,
    pub preferred_tools: Vec<String>,
}

/// Load all skills from .md files in the given directory.
pub fn load_skills(dir: &Path) -> Vec<Skill> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            eprintln!(
                "[default] WARN: could not read skills dir {}: {e}",
                dir.display()
            );
            return Vec::new();
        }
    };

    let name_re = Regex::new(r"(?i)^#\s*skill:\s*(.+)$").unwrap();
    let mut skills = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().map_or(true, |e| e != "md") {
            continue;
        }
        // Skip _always-on.md (it's injected separately)
        if path
            .file_name()
            .is_some_and(|n| n.to_string_lossy().starts_with('_'))
        {
            continue;
        }

        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[default] WARN: could not read skill {}: {e}", path.display());
                continue;
            }
        };

        let name = name_re
            .captures(&content)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().trim().to_string())
            .unwrap_or_else(|| {
                path.file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            });

        let trigger_keywords = extract_section(&content, "Cuándo activar")
            .map(|s| {
                s.split(", ")
                    .map(|k| k.trim().to_lowercase())
                    .filter(|k| !k.is_empty())
                    .collect()
            })
            .unwrap_or_default();

        let instructions = extract_section(&content, "Instrucciones").unwrap_or_default();

        let preferred_tools = extract_section(&content, "Tools preferidas")
            .map(|s| {
                s.lines()
                    .filter_map(|l| l.strip_prefix("- ").map(|t| t.trim().to_string()))
                    .collect()
            })
            .unwrap_or_default();

        skills.push(Skill {
            name,
            trigger_keywords,
            instructions,
            preferred_tools,
        });
    }

    eprintln!("[default] loaded {} skills", skills.len());
    skills
}

/// Match skills against user text by keyword substring matching.
/// Returns up to `max_active` skills sorted by match count (descending).
pub fn match_skills<'a>(skills: &'a [Skill], user_text: &str, max_active: usize) -> Vec<&'a Skill> {
    let lower = user_text.to_lowercase();

    let mut scored: Vec<(&Skill, usize)> = skills
        .iter()
        .map(|s| {
            let count = s
                .trigger_keywords
                .iter()
                .filter(|kw| lower.contains(kw.as_str()))
                .count();
            (s, count)
        })
        .filter(|(_, count)| *count > 0)
        .collect();

    scored.sort_by_key(|b| std::cmp::Reverse(b.1));
    scored
        .into_iter()
        .take(max_active)
        .map(|(s, _)| s)
        .collect()
}

/// Extract a section from markdown by its ## header name.
/// Returns the content between the header and the next ## header (or EOF).
fn extract_section(content: &str, section_name: &str) -> Option<String> {
    let header_pattern = format!("## {}", section_name);
    let lines: Vec<&str> = content.lines().collect();

    let start = lines
        .iter()
        .position(|l| l.trim().starts_with(&header_pattern))?;

    let end = lines
        .iter()
        .skip(start + 1)
        .position(|l| l.starts_with("## "))
        .map(|p| p + start + 1)
        .unwrap_or(lines.len());

    let section: String = lines[start + 1..end].to_vec().join("\n").trim().to_string();

    if section.is_empty() {
        None
    } else {
        Some(section)
    }
}
