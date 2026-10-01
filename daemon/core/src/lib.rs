pub mod approval;
pub mod auth;
pub mod clients;
pub mod config;
pub mod env_catalog;
pub mod env_watcher;
pub mod errors;
pub mod environments;
pub mod events;
pub mod llm_admin;
pub mod local_models;
pub mod plugins;
pub mod protocols;
pub mod registry;
pub mod router;

pub mod runtime;
pub mod session;
pub mod session_agents;
pub mod skills;
pub mod startup;
pub mod spawner;
pub mod supervisor;
pub mod tasks;

pub use config::CoreConfig;
// EP-2026-08-19: the tool registry moved to the `tools-engine`
// workspace member (see `daemon/tools-engine/`). The old
// `core/src/tools/` module was deleted. The daemon consults
// `tools_engine::Engine` directly via the `AppState.engine` field.
// The old HTTP proxy client (`llmd_client`) was also deleted; the
// daemon talks to llmd functionality in-process now.

