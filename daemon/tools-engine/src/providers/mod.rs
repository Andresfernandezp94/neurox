//! LLM provider CRUD and discovery. F5.1: the SQLite-backed store
//! replaces the daemon's `core/src/llm_providers/store.rs` — the
//! daemon now consults the engine directly.

pub mod model_config_store;
pub mod model_downloader;
pub mod models_discovery;
pub mod provider_store;
pub mod settings;
pub mod store;
pub mod user_llm_pref;
