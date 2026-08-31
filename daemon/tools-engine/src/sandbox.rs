//! Sandbox configuration trait for tool execution. Used by tools to
//! decide whether a path is allowed for read/write operations.
//!
//! EP-0015: unifies the previous duplicated `tools_engine::SandboxConfig`
//! struct (kept here to avoid core dependency) and `core::config::SandboxConfig`
//! into a single trait. The Engine stores `Arc<RwLock<Box<dyn SandboxConfig>>>`
//! so runtime updates (via `PUT /v1/sandbox`) take effect on the next tool call.

use std::path::{Path, PathBuf};

/// Trait that every sandbox config must implement. The default
/// implementation is `DefaultSandbox` (locked-down: nothing writable,
/// nothing readable). Production usage wires `core::config::SandboxConfig`
/// which is loaded from `~/.config/neurox/config.yaml`.
pub trait SandboxConfig: Send + Sync {
    /// Whether the sandbox is enforced at all. When false, all
    /// filesystem tools are unrestricted.
    fn enabled(&self) -> bool;

    /// Whether `path` is inside a writable root. Write tools (write_file,
    /// shell) check this before touching the filesystem.
    fn is_writable(&self, path: &Path) -> bool;

    /// Whether `path` is inside a readable root. Read tools (read_file,
    /// list_dir, grep, glob) check this. Read+write is implied:
    /// `is_writable(p)` implies `is_readable(p)`.
    fn is_readable(&self, path: &Path) -> bool;

    /// Optional max recursion depth for glob/grep. Default `None` = use
    /// the tool's built-in default.
    fn max_recursion_depth(&self) -> Option<usize> {
        None
    }

    /// Resolve `writable_paths` strings against `workspace_root`. Honors
    /// the `${workspace}` placeholder. Default impl returns the
    /// raw strings (no resolution).
    fn writable_paths_resolved(&self, _workspace_root: &Path) -> Vec<PathBuf> {
        Vec::new()
    }

    /// Resolve `readable_paths` strings (used by read tools). Default
    /// impl returns raw strings.
    fn readable_paths_resolved(&self, _workspace_root: &Path) -> Vec<PathBuf> {
        Vec::new()
    }
}

/// Default sandbox: nothing is writable, nothing is readable, but
/// the engine still tracks the flag. Used when no config is provided.
#[derive(Debug, Clone, Default)]
pub struct DefaultSandbox;

impl SandboxConfig for DefaultSandbox {
    fn enabled(&self) -> bool {
        false
    }
    fn is_writable(&self, _path: &Path) -> bool {
        false
    }
    fn is_readable(&self, _path: &Path) -> bool {
        false
    }
}
