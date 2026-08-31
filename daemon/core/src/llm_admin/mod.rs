//! LLM administration layer: catalog + local service orchestration.
//!
//! Lived in `core/src/llm_providers/` until EP-0004 wave 1. The provider
//! store and per-model discovery moved to `tools_engine::providers`.
//! What remains is the daemon's accumulated catalog cache and the local
//! LLM service orchestrator (llama-server, Ollama, etc.).

pub mod catalog;
pub mod orchestrator;

pub use catalog::{
    build_catalog, invalidate_llm_catalog, kind_slug, CatalogEntry, ModelCatalog, CATALOG_TTL_SECS,
};
pub use orchestrator::{DesiredState, LocalServiceOrchestrator, LocalServiceStatus, ServiceState};