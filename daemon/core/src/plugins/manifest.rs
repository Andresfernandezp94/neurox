// Plugin manifest schema. Each plugin tarball contains a `manifest.json`
// at the root describing its metadata, capabilities, and dependencies.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub license: Option<String>,
    /// Repository URL where this plugin lives.
    pub repo: String,
    /// Tags for discoverability.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Semver constraint on the core neurox version. Strings parsed
    /// with `semver::VersionReq`. At least one of `min_core_version` or
    /// `core_version` should be set.
    #[serde(default)]
    pub min_core_version: Option<String>,
    #[serde(default)]
    pub max_core_version: Option<String>,
    /// Path to the executable inside the plugin directory, relative to
    /// the plugin root. Default: `bin/run`.
    #[serde(default = "default_entry")]
    pub entry: String,
    /// Capabilities this plugin contributes. The core does not
    /// interpret these — it just exposes the list in
    /// `GET /v1/services` for clients/UIs to read.
    #[serde(default)]
    pub capabilities: Vec<PluginCapability>,
    /// Tools this plugin provides. EP-0005. The core's tool registry
    /// will dispatch calls for these names to the plugin's HTTP endpoint
    /// (see `http_base_url` or `socket_base_url`).
    #[serde(default)]
    pub tools: Vec<PluginTool>,
    /// Native libraries the plugin ships. Paths relative to the plugin
    /// root; the runner sets `LD_LIBRARY_PATH` (or platform equivalent)
    /// before exec.
    #[serde(default)]
    pub native_libs: Vec<String>,
    /// Resource paths the plugin exposes (configs, schemas, etc) —
    /// documented for humans, not consumed by the core.
    #[serde(default)]
    pub resources: BTreeMap<String, String>,
}

impl PluginManifest {
    pub fn entry_path(&self) -> &str {
        if self.entry.is_empty() {
            "bin/run"
        } else {
            &self.entry
        }
    }
}

fn default_entry() -> String {
    "bin/run".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PluginCapability {
    /// Provides a web UI accessible via HTTP.
    WebUi { url: String },
    /// Provides a terminal UI.
    TerminalUi,
    /// Provides a CLI binary.
    Cli { command: String },
    /// Provides a daemon/service (e.g. memory backend).
    Service {
        socket: Option<String>,
        http: Option<String>,
    },
    /// Adds new endpoints to the core (path prefix).
    AddsEndpoints { prefix: String },
    /// Adds events to the broadcast channel.
    AddsEvents,
    /// Arbitrary string for clients to interpret.
    Custom {
        name: String,
        payload: serde_json::Value,
    },
}

/// A tool provided by a plugin. EP-0005.
/// The core's tool registry dispatches calls to the plugin's HTTP endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginTool {
    pub name: String,
    pub description: String,
    /// HTTP method. EP-0005 supports POST (with JSON body) and GET.
    #[serde(default = "default_tool_method")]
    pub method: String,
    /// Path relative to the plugin's HTTP base URL (e.g. "/memory/recall").
    pub path: String,
    /// JSON Schema for the parameters.
    #[serde(default)]
    pub parameters: serde_json::Value,
    /// Requires user approval before exec (like core tools).
    #[serde(default)]
    pub requires_approval: bool,
}

fn default_tool_method() -> String {
    "POST".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginVersion {
    pub semver: String,
    pub min_core_version: Option<String>,
    pub max_core_version: Option<String>,
}
