use clap::{Parser, Subcommand};
use std::sync::Arc;
use tracing::{error, info};

use neurox::{
    approval, llm_admin::LocalServiceOrchestrator, registry::Registry,
    router::state::{
        AppState, AuthLayer, EventsLayer, LifecycleLayer, WorkspaceLayer,
    },
    runtime, skills::SkillsRegistry, spawner::Spawner, supervisor::Supervisor, startup, tasks,
    CoreConfig,
};
use tokio::sync::broadcast;

#[derive(Parser)]
#[command(name = "neurox")]
#[command(version, about = "Headless daemon for orchestrating AI agents", long_about = None)]
struct Cli {
    #[arg(long, global = true, default_value = "info")]
    log_level: String,

    #[arg(long, global = true)]
    config: Option<std::path::PathBuf>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Serve {
        #[arg(long, default_value = "127.0.0.1:7878")]
        bind: String,
    },
    Agents {
        #[command(subcommand)]
        action: AgentsCmd,
    },
    Plugin {
        #[command(subcommand)]
        action: PluginCmd,
    },
    /// Manage LLM providers (EP-0010).
    Providers {
        #[command(subcommand)]
        action: ProvidersCmd,
    },
    /// Manage users (EP-0007).
    Users {
        #[command(subcommand)]
        action: UsersCmd,
    },
}

#[derive(Subcommand)]
enum AgentsCmd {
    List,
    Start { id: String },
    Stop { id: String },
}

#[derive(Subcommand)]
enum PluginCmd {
    /// Install a plugin from the registry.
    Install {
        name: String,
        #[arg(long, default_value = "latest")]
        version: String,
    },
    /// List installed plugins.
    List,
    /// Run a plugin in the foreground. Blocks until the plugin exits.
    Run { name: String, args: Vec<String> },
    /// Update a plugin to the latest version.
    Update { name: String },
    /// Remove an installed plugin.
    Remove { name: String },
    /// Generate a systemd user service for the plugin.
    Service { name: String },
}

#[derive(Subcommand)]
enum ProvidersCmd {
    /// List configured LLM providers from SQLite.
    List,
    /// Re-import providers from config.yaml into SQLite (overwrites existing).
    SyncFromYaml,
}

/// EP-0007: user management commands. Operates directly on the JSON
/// user store at `~/.config/neurox/users.json`. The daemon does NOT
/// need to be running for these commands.
#[derive(Subcommand)]
enum UsersCmd {
    /// Reset a user's password to a random 16-char value. Prints the
    /// new password to stdout. Use this when the admin is locked out.
    ResetPassword {
        /// Username to reset. If omitted, defaults to "andres.fernandez".
        #[arg(long, default_value = "andres.fernandez")]
        username: String,
    },
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // EP-0012 O-002 + O-004 + O-005: request_id correlation + JSON formatter
    // + span events. JSON formatter activates when NEUROX_LOG_FORMAT=json
    // (for ingestion by Loki/Datadog/CloudWatch); plaintext otherwise.
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&cli.log_level));
    let is_json = std::env::var("NEUROX_LOG_FORMAT").as_deref() == Ok("json");
    let builder = tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE);
    if is_json {
        builder.json().init();
    } else {
        builder.init();
    }

    info!("neurox v{} starting", env!("CARGO_PKG_VERSION"));

    // Warn if the default will be unable to call the LLM. The default module
    // degrades gracefully (returns errors on LLM calls) but it's better
    // to surface this here than to fail mysteriously on first message.
    if std::env::var("MINIMAX_API_KEY").is_err() {
        tracing::warn!(
            "MINIMAX_API_KEY is not set. The default will start, but \
             LLM calls (default process) will return an error. \
             Set the env var (and MINIMAX_BASE_URL / MINIMAX_MODEL if needed) \
             and restart the daemon to enable the default."
        );
    }

    runtime::init_dirs()?;

    let config_path = cli
        .config
        .clone()
        .unwrap_or_else(runtime::default_config_path);

    let core_config = CoreConfig::load(&config_path)?;
    info!("config loaded from {:?}", config_path);

    let bind = match &cli.command {
        Some(Commands::Serve { bind }) => bind.clone(),
        _ => core_config.bind_addr.clone(),
    };

    match cli.command.unwrap_or(Commands::Serve { bind }) {
        Commands::Serve { bind } => {
            serve(core_config, config_path, bind).await?;
        }
        Commands::Agents { action } => {
            handle_agents_action(core_config, action).await?;
        }
        Commands::Plugin { action } => {
            handle_plugin_action(core_config, action).await?;
        }
        Commands::Providers { action } => {
            handle_providers_action(core_config, action).await?;
        }
        Commands::Users { action } => {
            handle_users_action(action).await?;
        }
    }

    Ok(())
}

async fn serve(
    core_config: CoreConfig,
    config_path: std::path::PathBuf,
    bind: String,
) -> anyhow::Result<()> {
    info!("binding to {}", bind);

    let registry = Arc::new(Registry::new(config_path));
    registry.load_from_config(&core_config).await?;

// EP-2026-08-15: agent ids now come exclusively from `session_agents`
    // config and the runtime `POST /v1/agents/in_process` registry.
    // The legacy `in_process_default_agent_id` slot was removed in
    // EP-0004 wave 2; chat dispatch goes through `dispatch_to_agent`
    // → `SessionAgentPool`.
    info!("in-process default: legacy slot removed (EP-0004 wave 2)");

    // Local service orchestrator (EP-0018-02): starts/stops/supervises local
    // LLM services (e.g. llama-server) and reconciles their desired state.
    // Reads the active provider list from the engine (in-process).
    let local_orchestrator = Arc::new(LocalServiceOrchestrator::new());

    // Tool registry + provider CRUD + model configs + LLM backends
    // now live in `tools-engine` (was `mcps/llmd`). The daemon
    // constructs an `Engine` once and consults it directly — no
    // internal HTTP hop, no double registration.
    let workspace_root = std::env::var("NEUROX_WORKSPACE").map_or_else(
        |_| dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/")),
        std::path::PathBuf::from,
    );
    // EP-2026-08-19: tools-engine integration. The engine holds the
    // 17 native tools, the LLM provider store, and the model
    // discovery/downloader/configs. Constructing it once here replaces
    // the old `register_from_llmd` HTTP hop.
    let engine = startup::build_engine(&core_config, workspace_root.clone())
        .await?;
    let tools = engine.tools.clone();

    let llm_providers_for_orchestrator = engine.list_providers().await.unwrap_or_default();
    // EP-0004 wave 1: use the engine's load_llm_config helper instead of
    // hand-mapping provider fields. Returns an `LlmConfig` derived from
    // the engine's own types — the daemon's `LlmConfig` re-export is
    // built via `From` so callers don't have to know about both kinds.
    let engine_llm_config = engine.load_llm_config().await.unwrap_or_else(|_| {
        tools_engine::LlmConfig {
            default_provider: "minimax".to_string(),
            default_model: None,
            providers: vec![],
        }
    });
    let llm_config = neurox::config::LlmConfig {
        default_provider: engine_llm_config.default_provider,
        default_model: engine_llm_config.default_model,
        providers: engine_llm_config
            .providers
            .into_iter()
            .map(|p| neurox::config::LlmProviderConfig {
                id: p.id,
                kind: match p.kind {
                    tools_engine::LlmProviderKind::Minimax => {
                        neurox::config::LlmProviderKind::Minimax
                    }
                    tools_engine::LlmProviderKind::OpenaiCompat => {
                        neurox::config::LlmProviderKind::OpenaiCompat
                    }
                    tools_engine::LlmProviderKind::Anthropic => {
                        neurox::config::LlmProviderKind::Anthropic
                    }
                },
                base_url: p.base_url,
                model: p.model,
                api_key_env: p.api_key_env,
                extra: p.extra,
                local_command: p.local_command,
                local_args: p.local_args,
                local_model_path: p.local_model_path,
                local_port: p.local_port,
            })
            .collect(),
    };
    let _ = llm_providers_for_orchestrator; // suppressed; see engine_llm_config above
    local_orchestrator.startup_discover(&llm_config).await;
    {
        let orch = local_orchestrator.clone();
        tokio::spawn(LocalServiceOrchestrator::background_reconcile_loop(orch));
    }
    info!("local service orchestrator initialized");

    // Env-file watchdog (EP-0018-05): polls ~/.config/neurox/env every
    // 5s and propagates added/updated/removed keys to the daemon process
    // env. In-process consumers (default, tools) see new values immediately
    // on the next `std::env::var`. Subprocesses spawned AFTER the change
    // inherit the new env automatically. The `on_change` callback is
    // registered later, once `state` is constructed (so the catalog
    // cache can be invalidated when env vars change).

    let supervisor = Arc::new(Supervisor::new());
    let spawner = Arc::new(Spawner::new(core_config.spawner_concurrency));
    let tasks = Arc::new(tasks::TaskManager::new());
    let approvals = Arc::new(approval::ApprovalManager::default());

    // Per-session agent pool — one subprocess per chat session when the
    // session's `agent_id` matches a configured spec. Empty map means
    // no per-session spawning (legacy behaviour).
    let session_agents = Arc::new(neurox::session_agents::SessionAgentPool::new(
        core_config.session_agents.agents.clone(),
    ));
    info!(
        specs = session_agents.list_specs().await.len(),
        "session agent pool initialized"
    );

    // Dynamic plugin tool registry (EP-0009). Probes for running plugins
    // at startup and spawns a background health-check loop.
    let plugin_registry = Arc::new(neurox::plugins::PluginToolRegistry::new(tools.clone()));
    plugin_registry.startup_discover().await;
    {
        let pr = plugin_registry.clone();
        tokio::spawn(neurox::plugins::PluginToolRegistry::background_retry_loop(pr));
    }
    info!("plugin tool registry initialized");

    // Session store (SQLite) for persisting sessions + messages.
    let session = Arc::new(
        neurox::session::SessionStore::open(&core_config.db_path)
            .await
            .map_err(|e| anyhow::anyhow!("failed to open session store: {e}"))?,
    );
    info!(db_path = %core_config.db_path.display(), "session store opened");

    // EP-2026-08-19: install_template_context used to push the
    // sandbox + plugin caps into llmd via HTTP. The engine is
    // in-process now; tools read `state.workspace.sandbox` and the plugin
    // registry's live state on every call.
    let _sandbox = Arc::new(parking_lot::RwLock::new(core_config.sandbox.clone()));

    for spec in &core_config.agents.persistent {
        // The in-process default is handled by `core::default` directly,
        // not by the subprocess supervisor. Skip it.
        match supervisor.start_agent(spec.clone()).await {
            Ok(()) => info!(agent_id = %spec.id, "persistent agent started"),
            Err(e) => error!(
                agent_id = %spec.id,
                error = %e,
                "failed to start persistent agent"
            ),
        }
    }

    // We can't spawn the env_watcher until `state` exists (it owns the
    // catalog cache that the watcher needs to invalidate on env changes).
    // Defer the spawn until after AppState::new below.
    let lifecycle = Arc::new(LifecycleLayer::new(
        registry.clone(),
        supervisor.clone(),
        spawner.clone(),
        tasks.clone(),
        approvals.clone(),
        session_agents.clone(),
        session,
        Arc::new(SkillsRegistry::new()),
        plugin_registry,
    ));
    let workspace_sandbox: Arc<tokio::sync::RwLock<Box<dyn tools_engine::SandboxConfig>>> = Arc::new(
        tokio::sync::RwLock::new(Box::new(core_config.sandbox.clone())),
    );
    let workspace = Arc::new(WorkspaceLayer::new(workspace_root.clone(), workspace_sandbox));
    let (event_tx, _) = broadcast::channel(1024);
    let events = Arc::new(EventsLayer::new(event_tx));
    let auth = AuthLayer::new()
        .with_orchestrator(local_orchestrator.clone());

    let state = AppState::new(
        lifecycle,
        events,
        engine.clone(),
        auth,
        workspace,
        Arc::new(core_config.clone()),
    );

    // EP-2026-08-15: spawn one agent subprocess per InProcess spec.
    // Extracted into startup::spawn_in_process_agents (EP-2026-08-19
    // refactor of the original inline block).
    startup::spawn_in_process_agents(&state, &core_config);

    // EP-0007: when auth is enabled, build the AuthState (load/create
    // JWT secret, load user store, bootstrap admin if first run).
    let state = if core_config.auth.enabled {
        let auth_cfg = neurox::auth::AuthConfig::from_core(&core_config);
        let user_store = neurox::auth::UserStore::load(&auth_cfg.user_store_path)
            .map_err(|e| anyhow::anyhow!("failed to load user store: {e}"))?;
        let secret = std::sync::Arc::new(
            neurox::auth::JwtSecret::load_or_create(&auth_cfg.jwt_secret_path)
                .map_err(|e| anyhow::anyhow!("failed to load JWT secret: {e}"))?,
        );

        // First-run bootstrap: if the store is empty, create the admin user.
        // Password source (priority order):
        //   1. NEUROX_ADMIN_PASSWORD env var (fixed, no rotation)
        //   2. random 16-char password (printed once to stderr)
        // Set NEUROX_ADMIN_PASSWORD in ~/.config/neurox/env (or your systemd
        // unit) so the password is stable across reinstalls and never auto-rotates.
        if user_store.is_empty() {
            use neurox::auth::users::{hash_password, Role};
            let (admin_pwd, from_env) = match std::env::var("NEUROX_ADMIN_PASSWORD") {
                Ok(pwd) if !pwd.is_empty() => (pwd, true),
                _ => (generate_random_password(16), false),
            };
            user_store
                .create("andres.fernandez", &admin_pwd, Role::Admin)
            .map_err(|e| anyhow::anyhow!("failed to bootstrap admin: {e}"))?;
            // Hash then verify so the password hash is computed but the
            // cleartext never goes into logs / metrics.
            let _ = hash_password(&admin_pwd);
            eprintln!();
            eprintln!("═══════════════════════════════════════════════════════════════");
            eprintln!("  [auth] FIRST-RUN bootstrap");
            eprintln!("  Created initial admin user. Credentials:");
            eprintln!("    username: andres.fernandez");
            if from_env {
                eprintln!("    password: (from NEUROX_ADMIN_PASSWORD env var)");
            } else {
                eprintln!("    password: {admin_pwd}");
                eprintln!("  ⚠️  Save this password now. It will NOT be shown again.");
            }
            eprintln!("  Rotate via: PATCH /v1/users/me/password");
            eprintln!("  Or set NEUROX_ADMIN_PASSWORD to pin it on first run.");
            eprintln!("═══════════════════════════════════════════════════════════════");
            eprintln!();
        }

        let auth_state = neurox::auth::AuthState {
            user_store: std::sync::Arc::new(user_store),
            secret,
            expiry_hours: auth_cfg.jwt_expiry_hours,
            reauth_tokens: std::sync::Arc::new(neurox::auth::ReauthTokens::new()),
        };
        // Replace the auth layer with one that has the JWT secret.
        let mut state = state;
        state.auth = state.auth.with_auth(auth_state);
        state
    } else {
        state
    };
    // Spawn the env-watcher AFTER `state` exists so it can invalidate the
    // LLM catalog cache when env vars change. The watcher only needs the
    // cache (not the full AppState), so we pass it directly.
    let cache_for_watcher = state.workspace.llm_catalog_cache.clone();
    tokio::spawn(neurox::env_watcher::watch_loop(move || {
        let cache = cache_for_watcher.clone();
        async move {
            let mut guard = cache.lock().await;
            *guard = None;
        }
    }));
    // Idle-eviction sweeper for session agents. Kills subprocesses whose
    // `last_active` is older than the spec's `idle_timeout_secs`.
    {
        let pool = state.lifecycle.session_agents.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
            interval.tick().await; // skip the immediate first tick
            loop {
                interval.tick().await;
                pool.evict_idle().await;
            }
        });
    }
    let app = neurox::router::router(state);

    if let Some(tls) = &core_config.tls {
        info!(cert = %tls.cert.display(), key = %tls.key.display(), "starting with TLS");
        let config = axum_server::tls_rustls::RustlsConfig::from_pem_file(&tls.cert, &tls.key)
            .await
            .map_err(|e| anyhow::anyhow!("TLS config: {e}"))?;
        let handle = axum_server::Handle::<std::net::SocketAddr>::new();
        let handle_clone = handle.clone();
        tokio::spawn(async move {
            let _ = tokio::signal::ctrl_c().await;
            info!("shutdown signal received");
            handle_clone.graceful_shutdown(Some(std::time::Duration::from_secs(5)));
        });
        tasks.cancel_all().await;
        approvals.cancel_all().await;
        spawner
            .shutdown_all(std::time::Duration::from_secs(5))
            .await;
        supervisor.shutdown_all().await;
        session_agents.stop_all().await;
        local_orchestrator.shutdown_all().await;
        let addr: std::net::SocketAddr = bind.parse()?;
        let server = axum_server::bind_rustls(addr, config)
            .handle(handle)
            .serve(app.into_make_service_with_connect_info::<std::net::SocketAddr>());
        if let Err(e) = server.await {
            error!(error = %e, "server error");
            return Err(e.into());
        }
    } else {
        let listener = tokio::net::TcpListener::bind(&bind).await?;
        info!("listening on {}", bind);
        let server = axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .with_graceful_shutdown(async move {
            let _ = tokio::signal::ctrl_c().await;
            info!("shutdown signal received");
            tasks.cancel_all().await;
            approvals.cancel_all().await;
            spawner
                .shutdown_all(std::time::Duration::from_secs(5))
                .await;
            supervisor.shutdown_all().await;
            session_agents.stop_all().await;
            local_orchestrator.shutdown_all().await;
        });
        if let Err(e) = server.await {
            error!(error = %e, "server error");
            return Err(e.into());
        }
    }

    Ok(())
}

async fn handle_agents_action(core_config: CoreConfig, action: AgentsCmd) -> anyhow::Result<()> {
    use AgentsCmd::{List, Start, Stop};
    let registry = Arc::new(Registry::new(runtime::default_config_path()));
    registry.load_from_config(&core_config).await?;

    match action {
        List => {
            let persistent = registry.list_persistent().await;
            let ephemeral = registry.list_ephemeral_templates().await;
            println!("Persistent agents ({}):", persistent.len());
            for spec in persistent {
                println!("  - {} ({:?})", spec.id, spec.kind);
            }
            println!("\nEphemeral templates ({}):", ephemeral.len());
            for spec in ephemeral {
                println!("  - {} ({:?})", spec.id, spec.kind);
            }
        }
        Start { id } => {
            let spec = registry
                .get_persistent(&id)
                .await
                .ok_or_else(|| anyhow::anyhow!("agent not found: {id}"))?;
            let supervisor = Supervisor::new();
            supervisor.start_agent(spec).await?;
            println!("✓ started {id}");
        }
        Stop { id } => {
            let supervisor = Supervisor::new();
            supervisor.stop_agent(&id).await?;
            println!("✓ stopped {id}");
        }
    }
    Ok(())
}

async fn handle_plugin_action(core_config: CoreConfig, action: PluginCmd) -> anyhow::Result<()> {
    use PluginCmd::{Install, List, Remove, Run, Service, Update};
    let manager = neurox::plugins::PluginManager::default_local();

    let registry_url = std::env::var("NEUROX_REGISTRY")
        .ok()
        .or_else(|| core_config.plugins_registry.clone())
        .unwrap_or_else(|| {
            "https://raw.githubusercontent.com/Andresfernandezp94/neurox-registry/main/registry.json".to_string()
        });

    match action {
        Install { name, version } => {
            println!("Fetching registry from {}", registry_url);
            let registry = neurox::plugins::Registry::fetch(&registry_url).await?;
            let info = manager.install(&registry, &name, &version).await?;
            println!("✓ installed {}@{}", info.name, info.version);
            println!("  path: {}", info.install_path);
            println!("  run with: neurox run {}", info.name);
        }
        Update { name } => {
            let registry = neurox::plugins::Registry::fetch(&registry_url).await?;
            let info = manager.update(&registry, &name).await?;
            println!("✓ updated {} -> {}", name, info.version);
        }
        List => {
            let infos = manager.list()?;
            if infos.is_empty() {
                println!("No plugins installed.");
                println!("Use: neurox install <name>");
            } else {
                println!("Installed plugins ({}):", infos.len());
                println!("{:<14} {:<10} {:<12} PATH", "NAME", "VERSION", "STATUS");
                for info in infos {
                    let status = match info.status {
                        neurox::plugins::PluginInstallStatus::Ok => "ok",
                        neurox::plugins::PluginInstallStatus::Broken => "broken",
                        neurox::plugins::PluginInstallStatus::Incompatible => "incompatible",
                    };
                    println!(
                        "{:<14} {:<10} {:<12} {}",
                        info.name, info.version, status, info.install_path
                    );
                }
            }
        }
        Run { name, args } => {
            let code = manager.run(&name, &args)?;
            std::process::exit(code);
        }
        Remove { name } => {
            manager.remove(&name)?;
            println!("✓ removed {name}");
        }
        Service { name } => {
            let path = manager.service_unit(&name)?;
            println!("✓ wrote {}", path.display());
            println!("  enable + start with:");
            println!("    systemctl --user enable --now neurox-{name}.service");
        }
    }
    Ok(())
}

async fn handle_providers_action(
    core_config: CoreConfig,
    action: ProvidersCmd,
) -> anyhow::Result<()> {
    // EP-2026-08-19: the daemon sub-commands below operate on the
    // engine's SQLite directly. We open a dedicated engine handle
    // (not the AppState's) because the daemon may not be running.
    let workspace_root = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/"));
    let _ = workspace_root;
    let engine = tools_engine::Engine::new(
        &core_config.db_path,
        Arc::new(tokio::sync::RwLock::new(Box::new(tools_engine::DefaultSandbox) as Box<dyn tools_engine::SandboxConfig>)),
        workspace_root,
    )
    .await
    .map_err(|e| anyhow::anyhow!("engine init: {e}"))?;

    match action {
        ProvidersCmd::List => {
            let providers = engine
                .list_providers()
                .await
                .map_err(|e| anyhow::anyhow!("failed to list providers: {e}"))?;
            if providers.is_empty() {
                println!("No LLM providers configured.");
                println!(
                    "Use the REST API or `neurox providers sync-from-yaml` to add providers."
                );
            } else {
                println!(
                    "{:<16} {:<14} {:<40} {:<20} API_KEY_ENV",
                    "ID", "KIND", "BASE_URL", "MODEL"
                );
                for p in &providers {
                    println!(
                        "{:<16} {:<14} {:<40} {:<20} {}",
                        p.id,
                        p.kind.as_str(),
                        truncate_str(&p.effective_base_url(), 38),
                        truncate_str(&p.effective_model(), 18),
                        p.api_key_env.as_deref().unwrap_or("(none)"),
                    );
                }
                println!("\n{} provider(s) total.", providers.len());
            }
        }
        ProvidersCmd::SyncFromYaml => {
            let yaml_providers = &core_config.llm.providers;
            if yaml_providers.is_empty() {
                println!("No providers found in config.yaml `llm` section. Nothing to sync.");
                return Ok(());
            }

            // Clear existing providers and re-insert from YAML
            let existing = engine
                .list_providers()
                .await
                .map_err(|e| anyhow::anyhow!("failed to list providers: {e}"))?;
            for p in &existing {
                engine
                    .delete_provider(&p.id)
                    .await
                    .map_err(|e| anyhow::anyhow!("failed to delete provider '{}': {e}", p.id))?;
            }

            let mut imported = 0;
            for p in yaml_providers {
                let engine_provider = tools_engine::LlmProviderConfig {
                    id: p.id.clone(),
                    kind: match p.kind {
                        neurox::config::LlmProviderKind::Minimax => {
                            tools_engine::LlmProviderKind::Minimax
                        }
                        neurox::config::LlmProviderKind::OpenaiCompat => {
                            tools_engine::LlmProviderKind::OpenaiCompat
                        }
                        neurox::config::LlmProviderKind::Anthropic => {
                            tools_engine::LlmProviderKind::Anthropic
                        }
                    },
                    base_url: p.base_url.clone(),
                    model: p.model.clone(),
                    api_key_env: p.api_key_env.clone(),
                    extra: p.extra.clone(),
                    local_command: p.local_command.clone(),
                    local_args: p.local_args.clone(),
                    local_model_path: p.local_model_path.clone(),
                    local_port: p.local_port,
                };
                engine
                    .create_provider(&engine_provider)
                    .await
                    .map_err(|e| anyhow::anyhow!("failed to insert provider '{}': {e}", p.id))?;
                imported += 1;
                println!(
                    "  ✓ imported: {} ({})",
                    p.id,
                    engine_provider.kind.as_str()
                );
            }
            println!(
                "\n✓ Synced {} provider(s) from config.yaml to SQLite.",
                imported
            );
            if !existing.is_empty() {
                println!(
                    "  (replaced {} previously stored provider(s))",
                    existing.len()
                );
            }
        }
    }
    Ok(())
}

/// EP-0007: generate a strong random password for the bootstrap admin.
/// Uses `rand::thread_rng` over a charset with mixed alphanumeric + symbols.
fn generate_random_password(len: usize) -> String {
    use rand::seq::SliceRandom;
    const CHARSET: &[u8] =
        b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghjkmnpqrstuvwxyz23456789!@#$%^&*";
    let mut rng = rand::thread_rng();
    (0..len)
        .map(|_| *CHARSET.choose(&mut rng).unwrap())
        .map(|c| c as char)
        .collect()
}

fn truncate_str(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max - 1])
    }
}

/// EP-0007: user-management subcommands. Operates directly on the JSON
/// user store — the daemon does NOT need to be running.
async fn handle_users_action(action: UsersCmd) -> anyhow::Result<()> {
    use neurox::auth::{AuthConfig, UserStore};
    match action {
        UsersCmd::ResetPassword { username } => {
            let cfg = AuthConfig::default();
            let store = UserStore::load(&cfg.user_store_path)
                .map_err(|e| anyhow::anyhow!("failed to load user store: {e}"))?;
            let new_pwd = generate_random_password(16);
            // Find user by username (admin_reset_password takes id, so look up).
            let id = match store.find_by_username(&username) {
                Some(u) => u.id,
                None => {
                    anyhow::bail!(
                        "user '{username}' not found in {}",
                        cfg.user_store_path.display()
                    );
                }
            };
            store
                .admin_reset_password(id, &new_pwd)
                .map_err(|e| anyhow::anyhow!("failed to reset password: {e}"))?;
            eprintln!();
            eprintln!("═══════════════════════════════════════════════════════════════");
            eprintln!("  [auth] Password reset for user '{username}'");
            eprintln!("  New password: {new_pwd}");
            eprintln!("  ⚠️  Save it now. It will NOT be shown again.");
            eprintln!("  Change it immediately: PATCH /v1/users/me/password");
            eprintln!("═══════════════════════════════════════════════════════════════");
            eprintln!();
        }
    }
    Ok(())
}

