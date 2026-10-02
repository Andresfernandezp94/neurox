//! Dynamic plugin registration (EP-0009).
//!
//! Replaces the hardcoded memory-* tools with a generic HTTP-proxy
//! approach: any plugin registers via `POST /v1/mcps` declaring its
//! name, base URL, tools, and skills. The core:
//!
//! 1. Inserts `PluginProxyTool` instances into the `ToolRegistry`.
//! 2. Writes `.md` skill files to the default's skills dir.
//! 3. Tracks plugin state for reconnect/health.
//!
//! This module is purely about runtime registration. The tarball-based
//! install system (`manager.rs`, `registry.rs`) remains untouched.

use async_trait::async_trait;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn};

use tools_engine::tools::ToolSpec;
use tools_engine::{ExecuteContext, Tool, ToolRegistry};

// ─── Types ───────────────────────────────────────────────────────────────────

/// Payload for `POST /v1/mcps`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginRegisterRequest {
    /// Unique plugin id (e.g. "memory", "calendar").
    pub name: String,
    /// Base URL where the plugin listens (e.g. "http://127.0.0.1:9999").
    pub base_url: String,
    /// Tools the plugin exposes.
    #[serde(default)]
    pub tools: Vec<PluginToolDef>,
    /// Skills (markdown docs injected into the default context).
    #[serde(default)]
    pub skills: Vec<PluginSkillDef>,
    /// Optional health endpoint path (default: "/health").
    #[serde(default = "default_health_path")]
    pub health_path: String,
}

fn default_health_path() -> String {
    "/health".to_string()
}

/// A tool definition declared by the plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginToolDef {
    pub name: String,
    pub description: String,
    /// HTTP path relative to `base_url` (e.g. "/memory/recall").
    pub path: String,
    /// HTTP method (default: POST).
    #[serde(default = "default_method")]
    pub method: String,
    /// JSON Schema for the tool parameters.
    #[serde(default = "default_params_schema")]
    pub parameters: Value,
    /// Whether the tool requires user approval.
    #[serde(default)]
    pub requires_approval: bool,
}

fn default_method() -> String {
    "POST".to_string()
}

fn default_params_schema() -> Value {
    serde_json::json!({"type": "object", "properties": {}})
}

/// A skill definition — markdown content injected into the default's
/// skills directory so it becomes part of the system prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginSkillDef {
    /// Filename without extension (e.g. "memory-usage").
    pub name: String,
    /// Markdown content.
    pub content: String,
}

/// Runtime state of a registered plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginState {
    pub name: String,
    pub base_url: String,
    pub health_path: String,
    pub tools: Vec<String>,
    pub skills: Vec<String>,
    pub status: PluginStatus,
    pub registered_at: String,
    pub last_health_check: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginStatus {
    Connected,
    Disconnected,
    Unhealthy,
}

// ─── PluginProxyTool ─────────────────────────────────────────────────────────

/// Generic HTTP-proxy tool: forwards `execute(args)` as a JSON POST to
/// the plugin's endpoint and returns the response text.
pub struct PluginProxyTool {
    pub tool_name: String,
    pub description: String,
    pub parameters: Value,
    pub requires_approval: bool,
    pub base_url: String,
    pub path: String,
    pub method: String,
    /// PR-11: bearer token forwarded to the plugin's HTTP surface.
    /// Most plugins gated incoming requests with their own `--api-token`
    /// (e.g. memoryd, voiced); without forwarding it, all tool calls 401.
    /// `None` means no Authorization header is sent (e.g. for plugins that
    /// trust loopback or aren't auth-gated like guied).
    pub bearer: Option<String>,
    /// Workspace selected for this plugin connection. Sent as
    /// `X-Neurox-Workspace`; memoryd rejects MCP connections without it.
    pub workspace_header: Option<String>,
}

/// Resolve the bearer token to forward to a plugin by name. PR-11:
/// tries a few env-var conventions because the binary and the plugin
/// name don't always line up:
///
///   - `NEUROX_<NAME>_TOKEN`
///   - `NEUROX_<NAME>D_TOKEN`         (memoryd, voiced — old naming)
///   - `NEUROX_<NAME>_API_TOKEN`      (voiced legacy)
///   - `NEUROX_<NAME>D_API_TOKEN`     (voiced legacy + d-suffix)
///
/// Returns None if none is set or all are empty.
fn plugin_bearer_token(plugin_name: &str) -> Option<String> {
    let up = plugin_name.to_uppercase();
    for suffix in ["_TOKEN", "D_TOKEN", "_API_TOKEN", "D_API_TOKEN"] {
        let key = format!("NEUROX_{up}{suffix}");
        if let Ok(v) = std::env::var(&key) {
            if !v.is_empty() {
                tracing::debug!(plugin = %plugin_name, env = %key, "loaded plugin bearer token");
                return Some(v);
            }
        }
    }
    None
}

#[async_trait]
impl Tool for PluginProxyTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: self.tool_name.clone(),
            description: self.description.clone(),
            parameters: self.parameters.clone(),
            requires_approval: self.requires_approval,
            ..Default::default()
        }
    }

    async fn execute(&self, _ctx: &ExecuteContext, args: Value) -> Result<String, String> {
        let url = format!("{}{}", self.base_url.trim_end_matches('/'), self.path);
        // EP-2026-08-19: media tools can take a long time for the
        // upstream API call (image ~17s, music 1-3min, video 1-3min).
        // Default was 10s which caused spurious "unreachable" errors.
        let timeout_secs = match self.tool_name.as_str() {
            "generate_image" => 180,
            "generate_music" => 300,
            "generate_video" => 300,
            _ => 10,
        };
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .build()
            .map_err(|e| format!("http client: {e}"))?;

        let mut builder = match self.method.to_uppercase().as_str() {
            "GET" => client.get(&url).query(&[("args", args.to_string())]),
            _ => client.post(&url).json(&args),
        };
        if let Some(token) = &self.bearer {
            builder = builder.bearer_auth(token);
        }
        if let Some(workspace) = &self.workspace_header {
            builder = builder.header("X-Neurox-Workspace", workspace);
        }

        let resp = builder
            .send()
            .await
            .map_err(|e| format!("plugin '{}' unreachable at {}: {e}", self.tool_name, url))?;

        if !resp.status().is_success() {
            return Err(format!(
                "plugin '{}' returned HTTP {}",
                self.tool_name,
                resp.status()
            ));
        }

        let body = resp
            .text()
            .await
            .map_err(|e| format!("plugin '{}' read body: {e}", self.tool_name))?;

        Ok(body)
    }
}

// ─── PluginToolRegistry ──────────────────────────────────────────────────────

/// Tracks all dynamically-registered plugins and their state.
pub struct PluginToolRegistry {
    plugins: RwLock<HashMap<String, PluginState>>,
    tools: Arc<ToolRegistry>,
    skills_dir: PathBuf,
}

/// Shared handle to the plugin tool registry.
pub type SharedPluginToolRegistry = Arc<PluginToolRegistry>;

/// Resolve the skills directory, honouring `NEUROX_IDENTITY_DIR` and
/// falling back to `$XDG_DATA_HOME/neurox/identity`.
fn resolve_skills_dir() -> PathBuf {
    let base = std::env::var("NEUROX_IDENTITY_DIR")
        .ok()
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(fallback_skills_base);
    base.join("skills")
}

fn fallback_skills_base() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("neurox/identity")
}

impl PluginToolRegistry {
    /// Create a new registry. `tools` is the global tool registry where
    /// plugin tools will be inserted.
    ///
    /// Skills directory resolution:
    /// 1. `$NEUROX_IDENTITY_DIR/skills/` — canonical.
    /// 2. `$XDG_DATA_HOME/neurox/identity/skills/` — hardcoded fallback.
    pub fn new(tools: Arc<ToolRegistry>) -> Self {
        let skills_dir = resolve_skills_dir();
        Self {
            plugins: RwLock::new(HashMap::new()),
            tools,
            skills_dir,
        }
    }

    /// Register a plugin: add its tools to the global registry and write
    /// skill files to the default's skills dir. Replaces any previous
    /// registration with the same name.
    pub fn register(&self, req: PluginRegisterRequest) -> PluginState {
        // Unregister previous tools if plugin was already registered.
        self.unregister_tools(&req.name);

        // PR-11: resolve the bearer token once per plugin. Forwarded to
        // each tool's HTTP call so memoryd/voiced don't 401.
        let bearer = plugin_bearer_token(&req.name);
        let workspace_header = if req.name == "memory" {
            std::env::var("NEUROX_MEMORY_WORKSPACE")
                .or_else(|_| std::env::var("MEMORYD_WORKSPACE"))
                .ok()
                .filter(|value| !value.trim().is_empty())
        } else {
            None
        };

        let mut tool_names = Vec::new();
        for def in &req.tools {
            let proxy = PluginProxyTool {
                tool_name: def.name.clone(),
                description: def.description.clone(),
                parameters: def.parameters.clone(),
                requires_approval: def.requires_approval,
                base_url: req.base_url.clone(),
                path: def.path.clone(),
                method: def.method.clone(),
                bearer: bearer.clone(),
                workspace_header: workspace_header.clone(),
            };
            self.tools.register(Arc::new(proxy));
            tool_names.push(def.name.clone());
            debug!(tool = %def.name, plugin = %req.name, "registered plugin tool");
        }

        let skill_names: Vec<String> = req.skills.iter().map(|s| s.name.clone()).collect();
        self.reconcile_skills(&req.name, &req.skills);

        let state = PluginState {
            name: req.name.clone(),
            base_url: req.base_url.clone(),
            health_path: req.health_path.clone(),
            tools: tool_names,
            skills: skill_names,
            status: PluginStatus::Connected,
            registered_at: chrono::Utc::now().to_rfc3339(),
            last_health_check: None,
        };

        self.plugins.write().insert(req.name.clone(), state.clone());
        info!(plugin = %req.name, tools = req.tools.len(), skills = req.skills.len(), "plugin registered");
        state
    }

    /// Deregister an MCP: removes its tools from the global registry, deletes
    /// the entry from the in-memory plugin map, and emits
    /// `PluginUnregistered` via WS.
    ///
    /// Returns true if the MCP existed and was removed, false otherwise.
    pub fn remove(&self, plugin_name: &str) -> bool {
        let mut plugins = self.plugins.write();
        let Some(existing) = plugins.remove(plugin_name) else {
            return false;
        };
        let tools_removed = existing.tools.clone();
        drop(plugins);
        // Drop the guards before re-acquiring to avoid deadlock.
        for tool_name in &tools_removed {
            self.tools.unregister(tool_name);
        }
        info!(
            plugin = %plugin_name,
            tools = tools_removed.len(),
            "plugin unregistered"
        );
        true
    }

    /// Remove a plugin's tools from the global registry.
    fn unregister_tools(&self, plugin_name: &str) {
        let plugins = self.plugins.read();
        if let Some(existing) = plugins.get(plugin_name) {
            for tool_name in &existing.tools {
                self.tools.unregister(tool_name);
            }
        }
    }

    /// Write skill markdown files for a plugin. Removes any stale skill
    /// files from a previous registration of the same plugin.
    pub fn reconcile_skills(&self, plugin_name: &str, skills: &[PluginSkillDef]) {
        // Ensure skills directory exists.
        if let Err(e) = std::fs::create_dir_all(&self.skills_dir) {
            warn!(error = %e, "failed to create skills dir");
            return;
        }

        // Remove old skill files for this plugin.
        let prefix = format!("plugin-{plugin_name}-");
        if let Ok(entries) = std::fs::read_dir(&self.skills_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with(&prefix) && name.ends_with(".md") {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }

        // Write new skill files.
        for skill in skills {
            let filename = format!("plugin-{}-{}.md", plugin_name, skill.name);
            let path = self.skills_dir.join(&filename);
            if let Err(e) = std::fs::write(&path, &skill.content) {
                warn!(error = %e, file = %path.display(), "failed to write skill file");
            } else {
                debug!(file = %filename, "wrote plugin skill");
            }
        }
    }

    /// Attempt to reconnect to a plugin by probing its health endpoint.
    /// Returns true if the plugin is reachable.
    pub async fn reconnect(&self, plugin_name: &str) -> Result<(), String> {
        let (base_url, health_path) = {
            let plugins = self.plugins.read();
            let state = plugins
                .get(plugin_name)
                .ok_or_else(|| format!("mcp '{}' not registered", plugin_name))?;
            (state.base_url.clone(), state.health_path.clone())
        };

        let url = format!(
            "{}{}",
            base_url.trim_end_matches('/'),
            if health_path.starts_with('/') {
                health_path.clone()
            } else {
                format!("/{health_path}")
            }
        );

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(3))
            .build()
            .map_err(|e| format!("client: {e}"))?;

        let resp = client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("unreachable: {e}"))?;

        let now = chrono::Utc::now().to_rfc3339();
        let mut plugins = self.plugins.write();
        if let Some(state) = plugins.get_mut(plugin_name) {
            state.last_health_check = Some(now);
            if resp.status().is_success() {
                state.status = PluginStatus::Connected;
                info!(plugin = %plugin_name, "reconnected");
                Ok(())
            } else {
                state.status = PluginStatus::Unhealthy;
                Err(format!("health returned HTTP {}", resp.status()))
            }
        } else {
            Err(format!(
                "plugin '{}' disappeared during reconnect",
                plugin_name
            ))
        }
    }

    /// Startup discovery: probes well-known plugin ports/sockets and
    /// auto-registers any that respond with a manifest. This is a
    /// best-effort seed — plugins can always register later via the
    /// HTTP endpoint.
    pub async fn startup_discover(&self) {
        // Check if the legacy memory plugin is running.
        let memory_url = std::env::var("NEUROX_MEMORY_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:9999".to_string());

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap_or_default();

        let health_url = format!("{}/health", memory_url.trim_end_matches('/'));
        match client.get(&health_url).send().await {
            Ok(resp) if resp.status().is_success() => {
                info!(url = %memory_url, "discovered memory plugin at startup");
                // Try to fetch manifest/tools from the plugin.
                let manifest_url = format!("{}/manifest", memory_url.trim_end_matches('/'));
                if let Ok(manifest_resp) = client.get(&manifest_url).send().await {
                    if manifest_resp.status().is_success() {
                        if let Ok(req) = manifest_resp.json::<PluginRegisterRequest>().await {
                            self.register(req);
                            return;
                        }
                    }
                }
                // Fallback: register with default memory tools.
                let req = PluginRegisterRequest {
                    name: "memory".to_string(),
                    base_url: memory_url,
                    health_path: "/health".to_string(),
                    tools: vec![
                        PluginToolDef {
                            name: "recall".to_string(),
                            description: "Query the memory backend for context relevant to the current conversation.".to_string(),
                            path: "/memory/recall".to_string(),
                            method: "POST".to_string(),
                            parameters: serde_json::json!({
                                "type": "object",
                                "properties": {
                                    "query": {"type": "string", "description": "Search query"},
                                    "limit": {"type": "integer", "default": 5, "minimum": 1, "maximum": 20}
                                },
                                "required": ["query"]
                            }),
                            requires_approval: false,
                        },
                        PluginToolDef {
                            name: "store".to_string(),
                            description: "Store a memory entry. Categories: fact, learning, observation, open_loop, scope_rule.".to_string(),
                            path: "/memory/store".to_string(),
                            method: "POST".to_string(),
                            parameters: serde_json::json!({
                                "type": "object",
                                "properties": {
                                    "content": {"type": "string", "description": "The memory content to store"},
                                    "category": {"type": "string", "enum": ["fact", "learning", "observation", "open_loop", "scope_rule"], "default": "fact"}
                                },
                                "required": ["content"]
                            }),
                            requires_approval: false,
                        },
                        PluginToolDef {
                            name: "recent".to_string(),
                            description: "List recent memories chronologically (newest first).".to_string(),
                            path: "/memory/recent".to_string(),
                            method: "POST".to_string(),
                            parameters: serde_json::json!({
                                "type": "object",
                                "properties": {
                                    "category": {"type": "string", "enum": ["fact", "learning", "observation", "open_loop", "scope_rule", "all"], "default": "all"},
                                    "limit": {"type": "integer", "default": 10, "minimum": 1, "maximum": 100}
                                }
                            }),
                            requires_approval: false,
                        },
                        PluginToolDef {
                            name: "forget".to_string(),
                            description: "Delete a memory entry by id (irreversible).".to_string(),
                            path: "/memory/forget".to_string(),
                            method: "POST".to_string(),
                            parameters: serde_json::json!({
                                "type": "object",
                                "properties": {
                                    "id": {"type": "string", "description": "Memory id to delete"}
                                },
                                "required": ["id"]
                            }),
                            requires_approval: true,
                        },
                        PluginToolDef {
                            name: "workspace_list".to_string(),
                            description: "List registered workspace databases. Admin role required.".to_string(),
                            path: "/admin/workspaces".to_string(),
                            method: "GET".to_string(),
                            parameters: serde_json::json!({"type": "object", "properties": {}}),
                            requires_approval: false,
                        },
                        PluginToolDef {
                            name: "workspace_add".to_string(),
                            description: "Add or re-enable a workspace database. Admin role required.".to_string(),
                            path: "/admin/workspaces".to_string(),
                            method: "POST".to_string(),
                            parameters: serde_json::json!({
                                "type": "object",
                                "properties": {
                                    "id": {"type": "string", "description": "Workspace ID, e.g. workspace-1"},
                                    "name": {"type": "string", "description": "Human-readable workspace name"},
                                    "description": {"type": "string", "description": "Workspace description"}
                                },
                                "required": ["id"]
                            }),
                            requires_approval: false,
                        },
                        PluginToolDef {
                            name: "workspace_remove".to_string(),
                            description: "Disable a non-default workspace while retaining its database. Admin role required.".to_string(),
                            path: "/admin/workspaces/remove".to_string(),
                            method: "POST".to_string(),
                            parameters: serde_json::json!({
                                "type": "object",
                                "properties": {"id": {"type": "string"}},
                                "required": ["id"]
                            }),
                            requires_approval: false,
                        },
                    ],
                    skills: vec![],
                };
                self.register(req);
            }
            Ok(_) => {
                debug!("memory plugin responded but not healthy");
            }
            Err(_) => {
                debug!("memory plugin not running (startup discover)");
            }
        }
    }

    /// List all registered plugins.
    pub fn list(&self) -> Vec<PluginState> {
        self.plugins.read().values().cloned().collect()
    }

    /// Get a specific plugin state.
    pub fn get(&self, name: &str) -> Option<PluginState> {
        self.plugins.read().get(name).cloned()
    }

    /// Background retry task: periodically checks disconnected plugins
    /// and attempts reconnection.
    pub async fn background_retry_loop(registry: SharedPluginToolRegistry) {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            let disconnected: Vec<String> = {
                let plugins = registry.plugins.read();
                plugins
                    .iter()
                    .filter(|(_, s)| s.status == PluginStatus::Disconnected)
                    .map(|(name, _)| name.clone())
                    .collect()
            };
            for name in disconnected {
                if let Err(e) = registry.reconnect(&name).await {
                    debug!(plugin = %name, error = %e, "background retry failed");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tools_engine::tools::Tool;
    use tools_engine::tools::ToolRegistry;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[test]
    fn register_adds_tools_to_global_registry() {
        let tools = Arc::new(ToolRegistry::new());
        let plugin_reg = PluginToolRegistry::new(tools.clone());

        let req = PluginRegisterRequest {
            name: "test-plugin".into(),
            base_url: "http://127.0.0.1:1234".into(),
            health_path: "/health".into(),
            tools: vec![PluginToolDef {
                name: "test_tool".into(),
                description: "A test tool".into(),
                path: "/test".into(),
                method: "POST".into(),
                parameters: serde_json::json!({"type": "object"}),
                requires_approval: false,
            }],
            skills: vec![],
        };

        let state = plugin_reg.register(req);
        assert_eq!(state.status, PluginStatus::Connected);
        assert_eq!(state.tools, vec!["test_tool"]);
        assert!(tools.get("test_tool").is_some());
    }

    #[test]
    fn re_register_replaces_previous_tools() {
        let tools = Arc::new(ToolRegistry::new());
        let plugin_reg = PluginToolRegistry::new(tools.clone());

        let req1 = PluginRegisterRequest {
            name: "myplugin".into(),
            base_url: "http://127.0.0.1:1234".into(),
            health_path: "/health".into(),
            tools: vec![PluginToolDef {
                name: "old_tool".into(),
                description: "old".into(),
                path: "/old".into(),
                method: "POST".into(),
                parameters: serde_json::json!({"type": "object"}),
                requires_approval: false,
            }],
            skills: vec![],
        };
        plugin_reg.register(req1);
        assert!(tools.get("old_tool").is_some());

        let req2 = PluginRegisterRequest {
            name: "myplugin".into(),
            base_url: "http://127.0.0.1:1234".into(),
            health_path: "/health".into(),
            tools: vec![PluginToolDef {
                name: "new_tool".into(),
                description: "new".into(),
                path: "/new".into(),
                method: "POST".into(),
                parameters: serde_json::json!({"type": "object"}),
                requires_approval: false,
            }],
            skills: vec![],
        };
        plugin_reg.register(req2);
        assert!(
            tools.get("old_tool").is_none(),
            "old tool should be removed"
        );
        assert!(tools.get("new_tool").is_some());
    }

    #[tokio::test]
    async fn proxy_forwards_workspace_header() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buffer = vec![0; 4096];
            let bytes_read = stream.read(&mut buffer).await.unwrap();
            let request = String::from_utf8_lossy(&buffer[..bytes_read]);
            assert!(request
                .to_ascii_lowercase()
                .contains("x-neurox-workspace: workspace-7"));
            let body = b"{}";
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n",
                body.len()
            );
            stream.write_all(response.as_bytes()).await.unwrap();
            stream.write_all(body).await.unwrap();
        });

        let tool = PluginProxyTool {
            tool_name: "workspace_test".to_string(),
            description: "Test workspace header forwarding".to_string(),
            parameters: serde_json::json!({"type": "object"}),
            requires_approval: false,
            base_url: format!("http://{address}"),
            path: "/rpc".to_string(),
            method: "POST".to_string(),
            bearer: None,
            workspace_header: Some("workspace-7".to_string()),
        };
        Tool::execute(
            &tool,
            &tools_engine::tools::ExecuteContext {
                agent_id: "test-agent".to_string(),
                workspace: None,
            cancel: None,
                http_client: None,
            },
            serde_json::json!({}),
        )
        .await
        .unwrap();
        server.await.unwrap();
    }

    #[test]
    fn list_returns_all_plugins() {
        let tools = Arc::new(ToolRegistry::new());
        let plugin_reg = PluginToolRegistry::new(tools.clone());
        assert!(plugin_reg.list().is_empty());

        let req = PluginRegisterRequest {
            name: "p1".into(),
            base_url: "http://127.0.0.1:1111".into(),
            health_path: "/health".into(),
            tools: vec![],
            skills: vec![],
        };
        plugin_reg.register(req);
        assert_eq!(plugin_reg.list().len(), 1);
    }
}
