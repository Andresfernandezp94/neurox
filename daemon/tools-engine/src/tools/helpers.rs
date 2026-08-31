// Shared helpers used by tool implementations (extracted from the original
// core/src/tools/mod.rs and re-homed here so each tool file can use them).

use std::path::PathBuf;

pub fn resolve_under_workspace(
    workspace: &std::path::Path,
    path: &str,
    writable_paths: &[std::path::PathBuf],
    writable: bool,
) -> Result<PathBuf, String> {
    // 1. Tilde expansion (EP-0019-02)
    let expanded = expand_tilde(path)?;

    // 2. Build candidate
    let candidate = if std::path::Path::new(&expanded).is_absolute() {
        PathBuf::from(&expanded)
    } else {
        workspace.join(&expanded)
    };

    // 3. Normalize: resolve .. components without hitting the filesystem
    let mut normalized = PathBuf::new();
    for component in candidate.components() {
        match component {
            std::path::Component::ParentDir => {
                if !normalized.pop() {
                    return Err("path traversal: cannot go above root".to_string());
                }
            }
            std::path::Component::Normal(s) => normalized.push(s),
            std::path::Component::RootDir => normalized.push("/"),
            std::path::Component::CurDir => {}
            std::path::Component::Prefix(p) => normalized.push(p.as_os_str()),
        }
    }

    // 4. Under workspace → always allowed
    if normalized.starts_with(workspace) {
        return Ok(normalized);
    }

    // 5. Outside workspace → check writable_paths
    // Read-only ops may access explicitly listed paths; write ops must.
    if writable_paths.iter().any(|p| normalized.starts_with(p)) {
        return Ok(normalized);
    }

    if writable {
        Err(format!(
            "path '{}' resolves outside workspace '{}' and is not in writable_paths",
            path,
            workspace.display()
        ))
    } else {
        Err(format!(
            "path '{}' resolves outside workspace '{}' (not readable)",
            path,
            workspace.display()
        ))
    }
}

pub fn truncate_lines(text: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() <= max_lines {
        text.to_string()
    } else {
        let mut out: String = lines[..max_lines].join("\n");
        out.push_str(&format!(
            "\n\n[... truncated, showing {}/{} lines]",
            max_lines,
            lines.len()
        ));
        out
    }
}

pub fn grep_walk_dir(
    dir: &std::path::Path,
    re: &regex::Regex,
    max: usize,
    results: &mut Vec<String>,
) -> Result<(), String> {
    if results.len() >= max {
        return Ok(());
    }
    let entries = std::fs::read_dir(dir).map_err(|e| format!("readdir {}: {e}", dir.display()))?;
    for entry in entries.flatten() {
        if results.len() >= max {
            break;
        }
        let path = entry.path();
        let meta = entry.metadata().ok();
        if let Some(ref m) = meta {
            if m.is_dir() {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                // Skip hidden dirs and common noise
                if name.starts_with('.')
                    || name == "node_modules"
                    || name == "target"
                    || name == "__pycache__"
                {
                    continue;
                }
                grep_walk_dir(&path, re, max, results)?;
            } else if m.is_file() && m.len() < 1_000_000 {
                // Only search files < 1MB
                if let Ok(content) = std::fs::read_to_string(&path) {
                    for (i, line) in content.lines().enumerate() {
                        if results.len() >= max {
                            break;
                        }
                        if re.is_match(line) {
                            results.push(format!("{}:{}:{}", path.display(), i + 1, line));
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

pub fn glob_walk_dir(
    dir: &std::path::Path,
    pattern: &glob::Pattern,
    max: usize,
    base: &std::path::Path,
    results: &mut Vec<String>,
) {
    if results.len() >= max {
        return;
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        if results.len() >= max {
            break;
        }
        let path = entry.path();
        let meta = entry.metadata().ok();
        if let Some(ref m) = meta {
            // Get relative path for matching
            let rel = path.strip_prefix(base).unwrap_or(&path);
            let rel_str = rel.to_string_lossy();

            if m.is_dir() {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                if name.starts_with('.')
                    || name == "node_modules"
                    || name == "target"
                    || name == "__pycache__"
                {
                    continue;
                }
                glob_walk_dir(&path, pattern, max, base, results);
            } else if m.is_file() {
                // Match against filename or relative path
                let filename = path.file_name().unwrap_or_default().to_string_lossy();
                if pattern.matches(&filename) || pattern.matches(&rel_str) {
                    results.push(rel_str.to_string());
                }
            }
        }
    }
}

pub fn html_to_text(html: &str) -> String {
    let mut text = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut in_script = false;
    let mut in_style = false;

    let lower = html.to_lowercase();
    let chars: Vec<char> = html.chars().collect();
    let lower_chars: Vec<char> = lower.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if !in_tag && chars[i] == '<' {
            in_tag = true;
            // Check for script/style start/end
            let remaining: String = lower_chars[i..].iter().take(20).collect();
            if remaining.starts_with("<script") {
                in_script = true;
            } else if remaining.starts_with("</script") {
                in_script = false;
            } else if remaining.starts_with("<style") {
                in_style = true;
            } else if remaining.starts_with("</style") {
                in_style = false;
            }
            i += 1;
            continue;
        }
        if in_tag {
            if chars[i] == '>' {
                in_tag = false;
            }
            i += 1;
            continue;
        }
        if in_script || in_style {
            i += 1;
            continue;
        }
        // Decode entities
        if chars[i] == '&' {
            let rest: String = chars[i..].iter().take(10).collect();
            if rest.starts_with("&amp;") {
                text.push('&');
                i += 5;
            } else if rest.starts_with("&lt;") {
                text.push('<');
                i += 4;
            } else if rest.starts_with("&gt;") {
                text.push('>');
                i += 4;
            } else if rest.starts_with("&quot;") {
                text.push('"');
                i += 6;
            } else if rest.starts_with("&#39;") || rest.starts_with("&apos;") {
                text.push('\'');
                i += if rest.starts_with("&#39;") { 5 } else { 6 };
            } else if rest.starts_with("&nbsp;") {
                text.push(' ');
                i += 6;
            } else {
                text.push('&');
                i += 1;
            }
            continue;
        }
        text.push(chars[i]);
        i += 1;
    }

    // Collapse multiple blank lines
    let mut result = String::with_capacity(text.len());
    let mut blank_count = 0;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            blank_count += 1;
            if blank_count <= 2 {
                result.push('\n');
            }
        } else {
            blank_count = 0;
            result.push_str(trimmed);
            result.push('\n');
        }
    }
    result
}

pub fn expand_tilde(path: &str) -> Result<String, String> {
    if !path.starts_with('~') {
        return Ok(path.to_string());
    }
    // `~` or `~/` → home dir
    if path == "~" || path.starts_with("~/") {
        let home =
            dirs::home_dir().ok_or_else(|| "tilde expansion failed: HOME not set".to_string())?;
        let rest = path.trim_start_matches('~').trim_start_matches('/');
        if rest.is_empty() {
            return Ok(home.to_string_lossy().to_string());
        }
        return Ok(home.join(rest).to_string_lossy().to_string());
    }
    // `~user/` → look up via pwd (only if `~user` is at the start)
    if let Some(slash_pos) = path.find('/') {
        let user = &path[1..slash_pos];
        let _rest = &path[slash_pos + 1..];
        // Only attempt user lookup if `user` is non-empty and has no invalid chars
        if !user.is_empty()
            && user
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        {
            // Use getpwnam via libc. For simplicity, we only support `~user/`
            // if the user is the current user (i.e., we already expanded `~/`).
            // For arbitrary users, we fall through to the error case.
            return Err(format!(
                "tilde expansion of ~{} not supported (only ~/ is supported)",
                user
            ));
        }
    }
    Err(format!("invalid tilde path: {}", path))
}

