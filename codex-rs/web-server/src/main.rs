mod approval_manager;
mod attachments;
mod error;
mod event_stream;
mod handlers;
mod middleware;
mod state;

use anyhow::Context;
use axum::Json;
use axum::Router;
use axum::http::HeaderValue;
use axum::middleware::from_fn_with_state;
use axum::routing::get;
use axum::routing::patch;
use axum::routing::post;
use axum::routing::put;
use codex_core::ThreadManager;
use codex_core::auth::AuthManager;
use codex_core::config::service::ConfigService;
use codex_protocol::protocol::SessionSource;
use serde_json::Value;
use serde_json::json;
use std::sync::Arc;
use tower_http::cors::Any;
use tower_http::cors::CorsLayer;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;
use uuid::Uuid;

use crate::middleware::auth_middleware;
use crate::state::WebServerState;

#[derive(OpenApi)]
#[openapi(
    paths(
        handlers::create_thread,
        handlers::send_turn,
        handlers::stream_events,
        handlers::threads::create_thread,
        handlers::threads::list_threads,
        handlers::threads::archive_thread,
        handlers::threads::resume_thread,
        handlers::threads::fork_thread,
        handlers::turns::send_turn,
        handlers::turns::interrupt_turn,
        handlers::approvals::respond_to_approval,
        handlers::auth::login,
        handlers::auth::cancel_login,
        handlers::auth::logout,
        handlers::auth::get_account,
        handlers::auth::get_rate_limits,
        handlers::config::read_config,
        handlers::config::write_config_value,
        handlers::config::batch_write_config,
        handlers::config::read_config_requirements,
        handlers::models::list_models,
        handlers::skills::list_skills,
        handlers::skills::update_skill_config,
        handlers::mcp::list_mcp_server_status,
        handlers::mcp::refresh_mcp_servers,
        handlers::mcp::mcp_oauth_login,
        handlers::review::start_inline_review,
        handlers::review::start_detached_review,
        handlers::commands::execute_command,
        handlers::feedback::upload_feedback,
        attachments::upload_attachment,
        attachments::download_attachment,
    ),
    components(
        schemas(
            handlers::CreateThreadRequest,
            handlers::CreateThreadResponse,
            handlers::SendTurnRequest,
            handlers::SendTurnResponse,
            handlers::UserInputItem,
            handlers::threads::CreateThreadRequest,
            handlers::threads::CreateThreadResponse,
            handlers::threads::ListThreadsResponse,
            handlers::threads::ArchiveThreadResponse,
            handlers::turns::SendTurnRequest,
            handlers::turns::SendTurnResponse,
            handlers::turns::UserInputItem,
            handlers::turns::InterruptTurnRequest,
            handlers::turns::InterruptTurnResponse,
            handlers::approvals::ApprovalRequest,
            handlers::approvals::ApprovalResponse,
            handlers::auth::LoginRequest,
            handlers::auth::LoginResponse,
            handlers::auth::CancelLoginRequest,
            handlers::auth::CancelLoginResponse,
            handlers::auth::LogoutResponse,
            handlers::config::WriteConfigValueRequest,
            handlers::config::BatchWriteConfigRequest,
            handlers::config::WriteConfigResponse,
            attachments::UploadResponse,
            attachments::AttachmentMetadata,
        )
    ),
    tags(
        (name = "Threads", description = "Thread management endpoints"),
        (name = "Turns", description = "Turn submission and control endpoints"),
        (name = "Approvals", description = "Approval response endpoints"),
        (name = "Authentication", description = "User authentication endpoints"),
        (name = "Configuration", description = "Configuration management endpoints"),
        (name = "Models", description = "AI model listing endpoints"),
        (name = "Skills", description = "Skill management endpoints"),
        (name = "MCP", description = "MCP server management endpoints"),
        (name = "Review", description = "Code review endpoints"),
        (name = "Commands", description = "One-off command execution endpoints"),
        (name = "Feedback", description = "User feedback endpoints"),
        (name = "Events", description = "Event streaming endpoints"),
        (name = "Attachments", description = "File attachment endpoints"),
    ),
    info(
        title = "Codex Web Server API",
        version = "2.0.0",
        description = "HTTP REST API for Codex CLI - v1 (backward compatible) and v2 (enhanced) endpoints",
        contact(
            name = "Codex Team",
        )
    ),
    servers(
        (url = "http://127.0.0.1:8080", description = "Local server"),
        (url = "http://localhost:8080", description = "Local server (localhost)"),
    ),
    modifiers(&SecurityAddon)
)]
struct ApiDoc;

struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer_auth",
                utoipa::openapi::security::SecurityScheme::Http(
                    utoipa::openapi::security::Http::new(
                        utoipa::openapi::security::HttpAuthScheme::Bearer,
                    ),
                ),
            );
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let codex_home = dirs::home_dir()
        .context("Failed to get home dir")?
        .join(".codex");

    let attachments_dir = codex_home.join("attachments");
    std::fs::create_dir_all(&attachments_dir)?;

    let auth_token =
        std::env::var("CODEX_WEB_TOKEN").unwrap_or_else(|_| Uuid::new_v4().to_string());

    tracing::info!("🔐 Auth token: {}", auth_token);
    tracing::info!("🔗 Use: Authorization: Bearer {}", auth_token);

    let auth_manager = AuthManager::shared(
        codex_home.clone(),
        false,
        codex_core::auth::AuthCredentialsStoreMode::Keyring,
    );

    let config_service = Arc::new(ConfigService::new(
        codex_home.clone(),
        vec![],
        Default::default(),
    ));

    let thread_manager = Arc::new(ThreadManager::new(
        codex_home.clone(),
        auth_manager.clone(),
        SessionSource::VSCode,
    ));

    // Initialize CodexFeedback for feedback upload functionality
    let feedback = codex_feedback::CodexFeedback::new();

    let web_state = WebServerState::new(
        thread_manager,
        auth_manager,
        config_service,
        codex_home.clone(),
        attachments_dir,
        auth_token,
        feedback,
    );

    let protected_routes = Router::new()
        // v1 API (backward compatible)
        .route("/api/v1/threads", post(handlers::create_thread))
        .route("/api/v1/threads/{id}/turns", post(handlers::send_turn))
        .route("/api/v1/threads/{id}/events", get(handlers::stream_events))
        .route("/api/v1/attachments", post(attachments::upload_attachment))
        .route(
            "/api/v1/attachments/{id}",
            get(attachments::download_attachment),
        )
        // v2 API (new endpoints)
        .route("/api/v2/threads", post(handlers::threads::create_thread))
        .route("/api/v2/threads", get(handlers::threads::list_threads))
        .route(
            "/api/v2/threads/{id}/archive",
            post(handlers::threads::archive_thread),
        )
        .route(
            "/api/v2/threads/{id}/turns",
            post(handlers::turns::send_turn),
        )
        .route(
            "/api/v2/threads/{id}/turns/interrupt",
            post(handlers::turns::interrupt_turn),
        )
        .route(
            "/api/v2/threads/{thread_id}/approvals/{approval_id}",
            post(handlers::approvals::respond_to_approval),
        )
        .route("/api/v2/threads/{id}/events", get(handlers::stream_events))
        // Authentication endpoints
        .route("/api/v2/auth/login", post(handlers::auth::login))
        .route(
            "/api/v2/auth/login/cancel",
            post(handlers::auth::cancel_login),
        )
        .route("/api/v2/auth/logout", post(handlers::auth::logout))
        .route("/api/v2/auth/account", get(handlers::auth::get_account))
        .route(
            "/api/v2/auth/rate-limits",
            get(handlers::auth::get_rate_limits),
        )
        // Configuration endpoints
        .route("/api/v2/config", get(handlers::config::read_config))
        .route("/api/v2/config", put(handlers::config::write_config_value))
        .route(
            "/api/v2/config",
            patch(handlers::config::batch_write_config),
        )
        .route(
            "/api/v2/config/requirements",
            get(handlers::config::read_config_requirements),
        )
        // Models endpoints
        .route("/api/v2/models", get(handlers::models::list_models))
        // Skills endpoints
        .route("/api/v2/skills", get(handlers::skills::list_skills))
        .route(
            "/api/v2/skills/{name}",
            patch(handlers::skills::update_skill_config),
        )
        // MCP server endpoints
        .route(
            "/api/v2/mcp/servers",
            get(handlers::mcp::list_mcp_server_status),
        )
        .route(
            "/api/v2/mcp/servers/refresh",
            post(handlers::mcp::refresh_mcp_servers),
        )
        .route(
            "/api/v2/mcp/servers/{name}/auth",
            post(handlers::mcp::mcp_oauth_login),
        )
        // Review endpoints
        .route(
            "/api/v2/threads/{id}/reviews",
            post(handlers::review::start_inline_review),
        )
        .route(
            "/api/v2/reviews",
            post(handlers::review::start_detached_review),
        )
        // Commands endpoint
        .route(
            "/api/v2/commands",
            post(handlers::commands::execute_command),
        )
        // Feedback endpoint
        .route(
            "/api/v2/feedback",
            post(handlers::feedback::upload_feedback),
        )
        // Thread operations
        .route(
            "/api/v2/threads/{id}/resume",
            post(handlers::threads::resume_thread),
        )
        .route(
            "/api/v2/threads/{id}/fork",
            post(handlers::threads::fork_thread),
        )
        .layer(from_fn_with_state(web_state.clone(), auth_middleware));

    let app = Router::new()
        .route("/health", get(health))
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .merge(protected_routes)
        .layer(
            CorsLayer::new()
                .allow_origin([
                    HeaderValue::from_static("http://localhost:3000"),
                    HeaderValue::from_static("http://127.0.0.1:3000"),
                    HeaderValue::from_static("http://localhost:8080"),
                    HeaderValue::from_static("http://127.0.0.1:8080"),
                ])
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .with_state(web_state);

    let bind_addr =
        std::env::var("CODEX_WEB_BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".to_string());

    tracing::info!("🚀 Server starting on http://{}", bind_addr);
    tracing::info!("📚 Swagger UI: http://{}/swagger-ui", bind_addr);
    tracing::info!("📍 API v1 Endpoints (backward compatible):");
    tracing::info!("  GET  /health");
    tracing::info!("  POST /api/v1/threads");
    tracing::info!("  POST /api/v1/threads/{{id}}/turns");
    tracing::info!("  GET  /api/v1/threads/{{id}}/events (SSE)");
    tracing::info!("  POST /api/v1/attachments");
    tracing::info!("  GET  /api/v1/attachments/{{id}}");
    tracing::info!("📍 API v2 Endpoints (enhanced):");
    tracing::info!("  POST /api/v2/threads");
    tracing::info!("  GET  /api/v2/threads");
    tracing::info!("  POST /api/v2/threads/{{id}}/archive");
    tracing::info!("  POST /api/v2/threads/{{id}}/resume");
    tracing::info!("  POST /api/v2/threads/{{id}}/fork");
    tracing::info!("  POST /api/v2/threads/{{id}}/turns");
    tracing::info!("  POST /api/v2/threads/{{id}}/turns/interrupt");
    tracing::info!("  POST /api/v2/threads/{{thread_id}}/approvals/{{approval_id}}");
    tracing::info!("  GET  /api/v2/threads/{{id}}/events (SSE)");
    tracing::info!("  POST /api/v2/threads/{{id}}/reviews");
    tracing::info!("  POST /api/v2/reviews");
    tracing::info!("  POST /api/v2/auth/login");
    tracing::info!("  POST /api/v2/auth/login/cancel");
    tracing::info!("  POST /api/v2/auth/logout");
    tracing::info!("  GET  /api/v2/auth/account");
    tracing::info!("  GET  /api/v2/auth/rate-limits");
    tracing::info!("  GET  /api/v2/config");
    tracing::info!("  PUT  /api/v2/config");
    tracing::info!("  PATCH /api/v2/config");
    tracing::info!("  GET  /api/v2/config/requirements");
    tracing::info!("  GET  /api/v2/models");
    tracing::info!("  GET  /api/v2/skills");
    tracing::info!("  PATCH /api/v2/skills/{{name}}");
    tracing::info!("  GET  /api/v2/mcp/servers");
    tracing::info!("  POST /api/v2/mcp/servers/refresh");
    tracing::info!("  POST /api/v2/mcp/servers/{{name}}/auth");
    tracing::info!("  POST /api/v2/commands");
    tracing::info!("  POST /api/v2/feedback");

    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}
