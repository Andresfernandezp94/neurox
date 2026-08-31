//! Startup helpers — extracted from `main::serve` so the entrypoint stays
//! readable.
//!
//! EP-2026-08-19: previously these phases lived inline in `main.rs::serve`
//! which was 462 lines. The two heaviest blocks (engine construction +
//! in-process agent spawn) live here as `build_engine` and
//! `spawn_in_process_agents`.

use std::path::PathBuf;
use std::sync::Arc;

use tracing::{info, warn};

use crate::config::CoreConfig;
use tokio::sync::RwLock;

/// Convert the daemon's `LlmProviderConfig` (crate `neurox`) to the
/// engine's (crate `tools_engine`, same shape, different type).
pub fn engine_provider_from_config(
    p: &crate::config::LlmProviderConfig,
) -> tools_engine::LlmProviderConfig {
    use tools_engine::LlmProviderKind as EKind;
    let kind = match p.kind {
        crate::config::LlmProviderKind::Minimax => EKind::Minimax,
        crate::config::LlmProviderKind::OpenaiCompat => EKind::OpenaiCompat,
        crate::config::LlmProviderKind::Anthropic => EKind::Anthropic,
    };
    tools_engine::LlmProviderConfig {
        id: p.id.clone(),
        kind,
        base_url: p.base_url.clone(),
        model: p.model.clone(),
        api_key_env: p.api_key_env.clone(),
        extra: p.extra.clone(),
        local_command: p.local_command.clone(),
        local_args: p.local_args.clone(),
        local_model_path: p.local_model_path.clone(),
        local_port: p.local_port,
    }
}

/// Build the `tools-engine` engine: opens SQLite, registers the 17
/// native tools, and bootstraps providers from the YAML config.
///
/// Replaces the old `register_from_llmd` HTTP hop. The engine is now
/// in-process so there's no race condition with llmd startup.
pub async fn build_engine(
    core_config: &CoreConfig,
    workspace_root: PathBuf,
) -> anyhow::Result<Arc<tools_engine::Engine>> {
    // EP-0015 T-5: pass the configured sandbox to the engine, not the
    // default. The previous version used `tokio::sync::RwLock::new(Box::new(tools_engine::DefaultSandbox)) as Arc<tokio::sync::RwLock<Box<dyn tools_engine::SandboxConfig>>>`
    // which ignored the operator's config.yaml.
    let sandbox_cfg: Arc<RwLock<Box<dyn tools_engine::SandboxConfig>>> = Arc::new(
        RwLock::new(Box::new(core_config.sandbox.clone())),
    );
    let engine = tools_engine::Engine::new(
        &core_config.db_path,
        sandbox_cfg,
        workspace_root,
    )
    .await
    .map_err(|e| anyhow::anyhow!("engine init: {e}"))?;

    // Bootstrap provider list from YAML (first-run only).
    let yaml_providers: Vec<tools_engine::LlmProviderConfig> = core_config
        .llm
        .providers
        .iter()
        .map(engine_provider_from_config)
        .collect();
    let bootstrapped = engine
        .bootstrap_from_yaml(&yaml_providers)
        .await
        .map_err(|e| anyhow::anyhow!("LLM provider bootstrap failed: {e}"))?;
    if bootstrapped > 0 {
        info!(
            count = bootstrapped,
            "bootstrapped LLM providers from YAML to engine"
        );
    }
    Ok(engine)
}

/// Spawn one `agent` subprocess per `InProcessAgentSpec` from the
/// config (EP-2026-08-15 Phase 2). The daemon ships with the `agent`
/// binary in `target/release/agent`. In a release install we resolve
/// the binary from PATH or from CARGO_HOME.
pub fn spawn_in_process_agents(
    state: &crate::router::AppState,
    _core_config: &CoreConfig,
) {
    let agent_binary = std::env::var("NEUROX_AGENT_BIN")
        .ok()
        .map(PathBuf::from)
        .or_else(|| {
            // Look next to the daemon binary first, then fall back to PATH.
            let exe = std::env::current_exe().ok()?;
            let sibling = exe.parent()?.join("agent");
            if sibling.exists() {
                Some(sibling)
            } else {
                Some(PathBuf::from("agent"))
            }
        });
    let Some(bin) = agent_binary else {
        warn!("[main] no agent binary path resolved; in-process agents disabled");
        return;
    };
    let specs = state.config.resolved_in_process().to_vec();
    info!(
        binary = %bin.display(),
        count = specs.len(),
        "EP-2026-08-15: spawning in-process agent subprocesses (now via session_agents)"
    );
    let session_agents = state.lifecycle.session_agents.clone();
    let bin_clone = bin.clone();
    tokio::spawn(async move {
        if let Err(e) = session_agents
            .spawn_persistent_from_config(specs, &bin_clone)
            .await
        {
            warn!("[main] spawn_persistent_from_config reported: {e}");
        }
    });
}
