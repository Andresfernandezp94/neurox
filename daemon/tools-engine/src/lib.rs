//! tools-engine — the daemon's tool runtime and LLM provider manager.
//!
//! Replaces `mcps/llmd` (which was a separate process reachable at
//! `:9997`). Now the engine is a workspace member of the daemon repo
//! and is exposed directly to the HTTP handlers — no internal HTTP
//! hop, no double registration.
//!
//! What lives here:
//!
//! - `tools` — the 14 native tools (read / write / shell / todo /
//!   web) plus the `Tool` trait and `ToolRegistry`.
//! - `backend` — provider LLM backends (minimax, openai_compat,
//!   anthropic). Used by the engine's `/v1/providers/:id/test`
//!   endpoint and the model downloader. NOT used for chat (the
//!   agent subprocess continues to talk to MiniMax directly —
//!   separate EP).
//! - `providers` — SQLite-backed LLM provider CRUD
//!   (`LlmProviderConfig`), `/v1/models` discovery, HuggingFace
//!   model downloader, local service orchestration.
//! - `model_configs` — per-model inference config (SQLite).
//! - `sandbox` — path-level allowlist for filesystem / shell tools.
//!
//!   itself stays in the daemon's auth layer.
//! - `state` — `EngineState` (DB pool, tool registry, sandbox,
//!   workspace root, auth resolver).
//!
//! What does NOT live here (was here in `mcps/llmd`, deleted):
//!
//! - `agent/` — the LLM agent loop (`agent_loop`, `identity`,
//!   `memory`, `template`, `skills`). Dead code: the chat path
//!   routed through the subprocess, not llmd. Tracked as a
//!   follow-up under EP-2026-08-19.
//! - `http_tools.rs` — the axum HTTP server. The daemon owns
//!   that surface now.
//! - `lib.rs`/`main.rs`/`cli.rs` (`llmd` binary entrypoint). The
//!   daemon binary (`neurox`) is the only entrypoint.

pub mod backend;
pub mod config;
pub mod engine;
pub mod hf_models;
pub mod http_client;
pub mod model_configs;
pub mod providers;
pub mod sandbox;
pub mod state;
pub mod tools;

pub use config::{LlmConfig, LlmProviderConfig, LlmProviderKind};
pub use engine::Engine;
pub use http_client::{build as build_http_client, HttpClientConfig};
pub use providers::provider_store::ProviderStore;
pub use sandbox::{DefaultSandbox, SandboxConfig};
pub use state::EngineState;
pub use tools::{ExecuteContext, Tool, ToolRegistry, ToolSpec, ToolSpecWithState};
