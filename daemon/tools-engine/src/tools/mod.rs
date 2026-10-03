//! Tool registry + dispatcher.
//!
//! Tools are pure functions that agents can invoke. Each tool declares:
//! - a name (stable identifier used in tool calls)
//! - a description (shown to the LLM)
//! - a JSON schema for its parameters
//! - an async `execute` function
//!
//! The core keeps a global `ToolRegistry` and exposes it to the agent
//! dispatcher. When an agent emits a `tool_call`, the dispatcher looks
//! up the tool by name, validates the args against the schema, checks
//! the approval flow (if the tool is in `requires_approval`), executes
//! it, and returns the result as a `tool_result` event.

use std::collections::HashSet;

pub mod atomic_store;
pub mod read;
pub mod shell;
pub mod task_management;
pub mod url_safety;
pub mod write;
pub mod helpers;

use async_trait::async_trait;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock as TokioRwLock;

use crate::sandbox::SandboxConfig;

/// Category for declarative tool filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCategory {
    Filesystem, Web, Shell, Knowledge, TaskManagement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Build, Plan, Chat,
}

impl Mode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Mode::Build => "build",
            Mode::Plan => "plan",
            Mode::Chat => "chat",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub parameters: Value,
    #[serde(default)]
    pub requires_approval: bool,
    /// EP-2026-08-19: declarative categories.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub categories: Vec<ToolCategory>,
    /// EP-2026-08-19: modes the tool is compatible with.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mode_compatible: Vec<Mode>,
}

impl Default for ToolSpec {
    fn default() -> Self {
        Self {
            name: String::new(),
            description: String::new(),
            parameters: Value::Null,
            requires_approval: false,
            categories: Vec::new(),
            mode_compatible: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub args: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub id: String,
    pub ok: bool,
    pub output: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// EP-2026-08-19: per-request context passed to `Tool::execute`. Currently
/// only `agent_id` (so tools media proxied via the daemon can pick the
/// right output directory). The trait can be extended (session_id, etc.).
#[derive(Debug, Clone)]
pub struct ExecuteContext {
    pub agent_id: String,
    /// EP-0013 T-003: optional cancellation token. Tools that perform
    /// long-running operations (shell, generate_*) should
    /// poll `cancel.is_cancelled()` and return `Err("cancelled")` when
    /// triggered. Tools that don't poll still work, but won't be
    /// cancellable mid-execution.
    pub cancel: Option<tokio_util::sync::CancellationToken>,
    /// EP-0012 P-003: shared HTTP client. Tools that need to make HTTP
    /// calls should clone this instead of building their own
    /// `reqwest::Client` (which would skip connection pooling).
    pub http_client: Option<std::sync::Arc<reqwest::Client>>,
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn spec(&self) -> ToolSpec;
    fn categories(&self) -> Vec<ToolCategory> { vec![] }
    fn mode_compatible(&self) -> Vec<Mode> { vec![Mode::Build] }
    async fn execute(&self, ctx: &ExecuteContext, args: Value) -> Result<String, String>;
}

pub struct ToolRegistry {
    tools: RwLock<HashMap<String, Arc<dyn Tool>>>,
    /// EP-2026-08-19: decorated spec (categories + mode_compatible merged)
    /// cached at registration time.
    decorated_specs: RwLock<HashMap<String, Arc<ToolSpec>>>,
    /// Tools the operator has explicitly disabled via
    /// `set_enabled(name, false)`. Disabled tools stay registered but
    /// `get()` returns `None` for them.
    disabled: RwLock<std::collections::HashSet<String>>,
    /// EP-0014 C-002: optional path to a JSON sidecar that persists
    /// the `disabled` set across restarts. `None` = in-memory only.
    state_path: Option<std::path::PathBuf>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::with_state_path(None)
    }

    /// EP-0014 C-002: constructor with optional state persistence path.
    /// If `state_path` is `Some(p)`, `set_enabled` persists to `p`
    /// and `load_state` reads from it at startup.
    pub fn with_state_path(state_path: Option<std::path::PathBuf>) -> Self {
        Self {
            tools: RwLock::new(HashMap::new()),
            decorated_specs: RwLock::new(HashMap::new()),
            disabled: RwLock::new(std::collections::HashSet::new()),
            state_path,
        }
    }

    /// Register a tool. Replaces if a tool with the same name exists.
    ///
    /// EP-2026-08-19: builds the decorated spec (categories +
    /// mode_compatible merged) once at registration time and stores
    /// it in `decorated_specs`.
    pub fn register(&self, tool: Arc<dyn Tool>) {
        let mut spec = tool.spec();
        let categories = tool.categories();
        let mode_compatible = tool.mode_compatible();
        if !categories.is_empty() {
            spec.categories = categories;
        }
        if !mode_compatible.is_empty() {
            spec.mode_compatible = mode_compatible;
        }
        let name = spec.name.clone();
        let decorated = Arc::new(spec);
        self.decorated_specs.write().insert(name.clone(), decorated);
        self.tools.write().insert(name, tool);
    }

    pub fn unregister(&self, name: &str) -> bool {
        let removed = self.tools.write().remove(name).is_some();
        self.decorated_specs.write().remove(name);
        removed
    }

    /// Lookup a tool by name. Returns `None` if the tool has been
    /// disabled via `set_enabled(name, false)`.
    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        if !self.is_enabled(name) {
            return None;
        }
        self.tools.read().get(name).cloned()
    }

    /// List all registered tools as specs (for LLM function-calling).
    /// Disabled tools are still listed so callers can introspect them,
    /// but the caller should consult `is_enabled` before invoking.
    pub fn list_specs(&self) -> Vec<ToolSpec> {
        self.decorated_specs.read().values().map(|s| s.as_ref().clone()).collect()
    }

    /// Like `list_specs` but attaches the operator's enable flag to
    /// each entry. Used by `GET /v1/tools` so the frontend can render
    /// toggleable rows without a second round-trip.
    pub fn list_specs_detailed(&self) -> Vec<ToolSpecWithState> {
        let decorated = self.decorated_specs.read();
        let disabled = self.disabled.read();
        let mut out: Vec<ToolSpecWithState> = decorated
            .values()
            .map(|s| ToolSpecWithState {
                spec: s.as_ref().clone(),
                enabled: !disabled.contains(&s.name),
            })
            .collect();
        out.sort_by(|a, b| a.spec.name.cmp(&b.spec.name));
        out
    }

    /// List tool names that require approval.
    pub fn list_requiring_approval(&self) -> Vec<String> {
        self.decorated_specs
            .read()
            .values()
            .filter(|s| s.requires_approval)
            .map(|s| s.name.clone())
            .collect()
    }

    /// Set the operator-visible enable flag for a tool. Returns
    /// `true` if the tool is registered (the flag is stored either
    /// way — disabling a tool that hasn't been registered yet is a
    /// no-op for `get` until the tool appears, but the call still
    /// succeeds so the frontend doesn't have to know the registration
    /// order).
    ///
    /// EP-0014 C-002: persist to disk if `state_path` is set.
    pub fn set_enabled(&self, name: &str, enabled: bool) -> bool {
        let mut g = self.disabled.write();
        if enabled {
            g.remove(name);
        } else {
            g.insert(name.to_string());
        }
        // EP-0014 C-002: persist to sidecar JSON file.
        if let Some(path) = &self.state_path {
            if let Ok(content) = serde_json::to_string_pretty(&*g) {
                let _ = std::fs::write(path, content);
            }
        }
        self.tools.read().contains_key(name)
    }

    /// Returns the operator's enable flag. Defaults to `true` for
    /// tools that have never been toggled, so newly registered tools
    /// are callable out of the box.
    pub fn is_enabled(&self, name: &str) -> bool {
        !self.disabled.read().contains(name)
    }

    /// EP-0014 C-002: load persisted enable/disable state from disk.
    /// Called once at engine init. Missing file → empty state (no
    /// tools disabled by default).
    pub fn load_state(&self) -> std::io::Result<()> {
        if let Some(path) = &self.state_path {
            if path.exists() {
                let content = std::fs::read_to_string(path)?;
                let persisted: HashSet<String> = serde_json::from_str(&content)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                let mut g = self.disabled.write();
                *g = persisted;
            }
        }
        Ok(())
    }
}

/// Wrapper returned by `ToolRegistry::list_specs_detailed`. Pairs a
/// `ToolSpec` with the operator's enable flag so the frontend doesn't
/// have to look up the flag in a separate map.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ToolSpecWithState {
    #[serde(flatten)]
    pub spec: ToolSpec,
    pub enabled: bool,
}

pub type SharedToolRegistry = Arc<ToolRegistry>;

// === Built-in tools ===

// ---------------------------------------------------------------------------
// read_file
// ---------------------------------------------------------------------------

// EP-2026-08-19: `agent_id` now flows through `ExecuteContext` (the
// `Tool::execute` argument) instead of the previous process-global
// `set_agent_id()`. Tools that need it (e.g. media tools for output
// paths) read `ctx.agent_id` directly.

pub fn register_defaults(
    registry: &ToolRegistry,
    workspace_root: std::path::PathBuf,
    sandbox: Arc<TokioRwLock<Box<dyn SandboxConfig>>>,
) {
    // read/ — tools that need workspace_root + sandbox
    use crate::tools::read::glob::GlobTool;
    use crate::tools::read::grep::GrepTool;
    use crate::tools::read::list_dir::ListDirTool;
    use crate::tools::read::read_file::ReadFileTool;
    use crate::tools::read::symbols::SymbolsTool;
    use crate::tools::read::web_fetch::WebFetchTool;
    use crate::tools::read::web_search::WebSearchTool;
    // write/
    use crate::tools::write::write_file::WriteFileTool;
    // task_management/ — todo_* tools
    use crate::tools::task_management::todo_add::TodoAddTool;
    use crate::tools::task_management::todo_clear::TodoClearTool;
    use crate::tools::task_management::todo_done::TodoDoneTool;
    use crate::tools::task_management::todo_list::TodoListTool;
    use crate::tools::task_management::todo_remove::TodoRemoveTool;
    // shell/
    use crate::tools::shell::shell::ShellTool;

    // Tools that need workspace_root + sandbox context
    registry.register(Arc::new(ReadFileTool {
        workspace_root: workspace_root.clone(),
        sandbox: sandbox.clone(),
    }));
    registry.register(Arc::new(ListDirTool {
        workspace_root: workspace_root.clone(),
        sandbox: sandbox.clone(),
    }));
    registry.register(Arc::new(GrepTool {
        workspace_root: workspace_root.clone(),
        sandbox: sandbox.clone(),
        max_depth: 8,
    }));
    registry.register(Arc::new(GlobTool {
        workspace_root: workspace_root.clone(),
        sandbox: sandbox.clone(),
        max_depth: 8,
    }));
    registry.register(Arc::new(SymbolsTool {
        workspace_root: workspace_root.clone(),
        sandbox: sandbox.clone(),
    }));
    registry.register(Arc::new(WriteFileTool {
        workspace_root: workspace_root.clone(),
        sandbox: sandbox.clone(),
    }));
    registry.register(Arc::new(ShellTool {
        workspace_root: workspace_root.clone(),
        sandbox: sandbox.clone(),
        timeout_secs: 30,
    }));

    // Stateless tools (unit structs)
    registry.register(Arc::new(WebFetchTool));
    registry.register(Arc::new(WebSearchTool));
    // taskManagement/ — todo_* tools (stateless, file-backed)
    registry.register(Arc::new(TodoAddTool));
    registry.register(Arc::new(TodoClearTool));
    registry.register(Arc::new(TodoDoneTool));
    registry.register(Arc::new(TodoListTool));
    registry.register(Arc::new(TodoRemoveTool));
}

