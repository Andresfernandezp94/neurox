//! Routes composer — builds the axum Router from per-domain helpers.
//!
//! Each `*_routes()` function returns an `axum::Router<AppState>` with the
//! routes for one namespace. `composer::router()` composes them all.

use std::sync::Arc;

use axum::extract::Extension;

use super::AppState;
use super::http;
use super::ws;

#[allow(clippy::let_and_return)]
pub fn router(state: AppState) -> axum::Router {
    use axum::routing::{delete, get, post, put};
    let auth_token = state.auth.api_token.clone();

    let cors = tower_http::cors::CorsLayer::new()
        .allow_origin(tower_http::cors::Any)
        .allow_methods(tower_http::cors::Any)
        .allow_headers(tower_http::cors::Any);
    let api_routes = axum::Router::new()
        .route(
            "/v1/agents",
            get(http::list_agents).post(http::register_agent),
        )
        .route(
            "/v1/agents/in_process",
            post(http::register_in_process_agent),
        )
        .route(
            "/v1/agents/:id",
            get(http::get_agent)
                .delete(http::delete_agent)
                .patch(http::patch_agent),
        )
        .route(
            "/v1/agents/in_process/:id",
            delete(http::deregister_in_process_agent),
        )
        .route("/v1/agents/:id/start", post(http::start_agent))
        .route("/v1/agents/:id/stop", post(http::stop_agent))
        .route(
            "/v1/sessions",
            get(crate::router::handlers::sessions::list_sessions).post(http::create_session),
        )
        .route(
            "/v1/sessions/agents",
            get(http::list_session_agents),
        )
        .route(
            "/v1/sessions/:id",
            get(http::get_session).delete(http::delete_session),
        )
        .route(
            "/v1/sessions/:id/agent",
            get(http::get_session_agent),
        )
        .route(
            "/v1/sessions/:id/agent/restart",
            post(http::restart_session_agent),
        )
        .route(
            "/v1/sessions/:id/messages",
            get(crate::router::handlers::sessions::get_session_messages).post(http::post_message),
        )
        .route(
            "/v1/sessions/:id/messages/stream",
            post(http::post_message_stream),
        )
        .route("/v1/chat/raw", post(http::post_chat_raw))
        .route("/v1/sessions/:id/rename", put(http::rename_session))
        .route("/v1/sessions/:id/model", put(http::set_session_model))
        .route("/v1/sessions/:id/mode", put(http::set_session_mode))
        .route("/v1/sessions/:id/tool-mode", put(http::set_session_tool_mode))
        .route("/v1/sessions/:id/temperature", put(http::set_session_temperature))
        .route("/v1/sessions/:id/cancel", post(http::cancel_session))
        .route("/v1/approvals", get(http::list_approvals))
        .route("/v1/approvals/:id/respond", post(http::respond_approval))
        .route("/v1/events", get(ws::ws_events))
        .route("/v1/commands", get(ws::ws_commands))
        .route("/v1/tools", get(crate::router::handlers::tools_admin::list_tools))
        .route("/v1/tools/:name/invoke", post(crate::router::handlers::tools::invoke_tool))
        .route("/v1/tools/:name/enable", post(crate::router::handlers::tools_admin::enable_tool))
        .route("/v1/tools/:name/disable", post(crate::router::handlers::tools_admin::disable_tool))
        .route("/v1/services", get(http::list_services))
        .route(
            "/v1/llm/providers",
            get(http::list_llm_providers).post(http::create_llm_provider),
        )
        .route(
            "/v1/llm/providers/active",
            put(http::set_active_llm_provider),
        )
        .route("/v1/llm/providers/:id/test", post(http::test_llm_provider))
        .route("/v1/llm/providers/:id/ping", post(http::ping_llm_provider))
        .route(
            "/v1/llm/providers/:id/start",
            post(http::start_local_provider),
        )
        .route("/v1/files/*path", get(http::serve_file))
        .route(
            "/v1/llm/providers/:id/stop",
            post(http::stop_local_provider),
        )
        .route(
            "/v1/llm/providers/:id/models",
            get(http::list_provider_models),
        )
        .route("/v1/llm/models", get(http::get_llm_models))
        .route("/v1/llm/models/local", get(http::list_local_models))
        .route(
            "/v1/llm/models/local/configs",
            get(http::list_model_configs),
        )
        .route(
            "/v1/llm/models/local/:filename/config",
            get(http::get_model_config).put(http::put_model_config),
        )
        .route("/v1/llm/models/hf", get(http::search_hf_models))
        .route("/v1/llm/models/download", post(http::download_model))
        .route(
            "/v1/llm/providers/:id",
            put(http::update_llm_provider).delete(http::delete_llm_provider),
        )
        .route(
            "/v1/mcps",
            get(http::list_plugins).post(http::register_plugin),
        )
        .route(
            "/v1/mcps/:name",
            delete(http::unregister_mcp),
        )
        .route("/v1/mcps/:name/reconnect", post(http::reconnect_plugin))
        .route("/v1/mcps/catalog", get(http::list_plugins_catalog))
        .route("/v1/mcps/clean", post(http::clean_mcps))
        .route("/v1/env", get(http::list_env_vars))
        .route("/v1/env/:key", put(http::put_env_var).delete(http::delete_env_var))
        .route("/v1/sandbox", get(crate::router::handlers::sandbox::get_sandbox).put(crate::router::handlers::sandbox::put_sandbox));

    let auth_state = state.auth.auth.clone();
    let state = Arc::new(state);
    let api = if let Some(auth_state) = auth_state.clone() {
        let secret = auth_state.secret.clone();
        let public_paths = vec!["/health", "/livez", "/readyz", "/v1/auth/login"];
        axum::Router::new()
            .route("/health", get(http::health))
            .route("/livez", get(http::livez))
            .route("/readyz", get(http::readyz))
            .route("/v1/default/status", get(http::default_agent_status))
            .route("/v1/skills", get(http::list_skills_endpoint))
            .route("/v1/skills/:name/enable", post(crate::router::handlers::skills::enable_skill))
            .route("/v1/skills/:name/disable", post(crate::router::handlers::skills::disable_skill))
            .merge(api_routes)
            .merge(crate::auth::auth_routes().with_state(()))
            .with_state(state.clone())
            .merge(crate::auth::users_routes().with_state(()))
            .with_state(state.clone())
            .layer(axum::middleware::from_fn(
                crate::router::middleware::request_id::request_id_layer,
            ))
            .layer(crate::auth::JwtAuthLayer::new(secret, public_paths))
            .layer(Extension(auth_state))
    } else {
        let _ = auth_token;
        axum::Router::new()
            .route("/health", get(http::health))
            .route("/v1/default/status", get(http::default_agent_status))
            .route("/v1/skills", get(http::list_skills_endpoint))
            .route("/v1/skills/:name/enable", post(crate::router::handlers::skills::enable_skill))
            .route("/v1/skills/:name/disable", post(crate::router::handlers::skills::disable_skill))
            .merge(api_routes)
    };

    api.with_state(state).layer(cors)
}
