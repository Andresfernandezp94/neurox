// media capacity tools (one per file)
pub mod generate_image;
pub mod generate_music;
pub mod generate_video;

/// GI/GV-fix: sanitize a user-supplied filename to prevent path
/// traversal. The original code did `output_dir.join(filename)` without
/// validation, so an LLM could pass `../../tmp/pwned.jpg` and the
/// daemon would happily write wherever the user had filesystem access
/// (e.g. overwrite `~/.bashrc`, fill `/tmp` for DoS, etc.).
///
/// `default_ext` is appended when the input has no extension or has
/// one we don't allow.
pub fn sanitize_filename(input: &str, default_ext: &str) -> String {
    // Strip path components — keep only the basename.
    let basename = std::path::Path::new(input)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file");
    // Reject if the result is empty or `..` (after stripping).
    if basename.is_empty() || basename == "." || basename == ".." {
        return format!("file.{}", default_ext);
    }
    // Add default extension if missing.
    let has_ext = std::path::Path::new(basename)
        .extension()
        .is_some();
    if !has_ext && !default_ext.is_empty() {
        return format!("{}.{}", basename, default_ext);
    }
    basename.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_path_traversal() {
        assert_eq!(sanitize_filename("../../tmp/pwned.jpg", "jpg"), "pwned.jpg");
        assert_eq!(sanitize_filename("../../../../etc/passwd", "jpg"), "passwd.jpg");
        assert_eq!(sanitize_filename("/abs/path/foo.png", "jpg"), "foo.png");
        assert_eq!(sanitize_filename("./relative.txt", "jpg"), "relative.txt");
    }

    #[test]
    fn rejects_dot_and_dotdot() {
        assert_eq!(sanitize_filename(".", "jpg"), "file.jpg");
        assert_eq!(sanitize_filename("..", "jpg"), "file.jpg");
        assert_eq!(sanitize_filename("", "jpg"), "file.jpg");
    }

    #[test]
    fn adds_default_ext() {
        assert_eq!(sanitize_filename("foo", "jpg"), "foo.jpg");
        assert_eq!(sanitize_filename("foo.txt", "jpg"), "foo.txt");
    }
}
