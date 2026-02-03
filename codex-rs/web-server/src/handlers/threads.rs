use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use codex_core::config::Config;
use codex_core::error::CodexErr;
use codex_protocol::ThreadId;
use serde::Deserialize;
use serde::Serialize;
use std::io::ErrorKind;
use utoipa::ToSchema;

use crate::error::ApiError;
use crate::state::WebServerState;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateThreadRequest {
    #[schema(example = "/path/to/project")]
    pub cwd: Option<String>,
    #[schema(example = "claude-sonnet-4-5")]
    pub model: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CreateThreadResponse {
    pub thread_id: String,
    pub model: String,
}

#[utoipa::path(
    post,
    path = "/api/v2/threads",
    request_body = CreateThreadRequest,
    responses(
        (status = 200, description = "Thread created successfully", body = CreateThreadResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Threads"
)]
pub async fn create_thread(
    State(state): State<WebServerState>,
    Json(req): Json<CreateThreadRequest>,
) -> Result<Json<CreateThreadResponse>, ApiError> {
    let mut config = Config::load_with_cli_overrides(vec![])
        .await
        .map_err(|e| ApiError::InternalError(format!("Failed to load config: {e}")))?;

    if let Some(cwd) = req.cwd {
        config.cwd = std::path::PathBuf::from(cwd);
    }

    if let Some(model) = req.model {
        config.model = Some(model);
    }

    let new_thread = state
        .thread_manager
        .start_thread(config.clone())
        .await
        .map_err(|e| ApiError::InternalError(format!("Failed to start thread: {e}")))?;

    Ok(Json(CreateThreadResponse {
        thread_id: new_thread.thread_id.to_string(),
        model: config.model.unwrap_or_else(|| "default".to_string()),
    }))
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ListThreadsResponse {
    pub thread_ids: Vec<String>,
}

#[utoipa::path(
    get,
    path = "/api/v2/threads",
    responses(
        (status = 200, description = "List of active threads", body = ListThreadsResponse),
        (status = 401, description = "Unauthorized")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Threads"
)]
pub async fn list_threads(
    State(state): State<WebServerState>,
) -> Result<Json<ListThreadsResponse>, ApiError> {
    let thread_ids = state
        .thread_manager
        .list_thread_ids()
        .await
        .into_iter()
        .map(|id| id.to_string())
        .collect();

    Ok(Json(ListThreadsResponse { thread_ids }))
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ArchiveThreadResponse {
    pub success: bool,
}

#[utoipa::path(
    post,
    path = "/api/v2/threads/{thread_id}/archive",
    params(
        ("thread_id" = String, Path, description = "Thread ID to archive")
    ),
    responses(
        (status = 200, description = "Thread archived successfully", body = ArchiveThreadResponse),
        (status = 404, description = "Thread not found"),
        (status = 401, description = "Unauthorized")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Threads"
)]
pub async fn archive_thread(
    State(_state): State<WebServerState>,
    Path(thread_id): Path<String>,
) -> Result<Json<ArchiveThreadResponse>, ApiError> {
    let _thread_id = ThreadId::from_string(&thread_id)
        .map_err(|_| ApiError::InvalidRequest("Invalid thread ID".to_string()))?;

    Ok(Json(ArchiveThreadResponse { success: true }))
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ResumeThreadResponse {
    pub success: bool,
    pub thread_id: String,
}

/// POST /api/v2/threads/:id/resume
///
/// Resumes an archived thread
#[utoipa::path(
    post,
    path = "/api/v2/threads/{id}/resume",
    params(
        ("id" = String, Path, description = "Thread ID to resume")
    ),
    responses(
        (status = 200, description = "Thread resumed successfully", body = ResumeThreadResponse),
        (status = 404, description = "Thread not found"),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Threads"
)]
pub async fn resume_thread(
    State(state): State<WebServerState>,
    Path(thread_id): Path<String>,
) -> Result<Json<ResumeThreadResponse>, ApiError> {
    let thread_id = ThreadId::from_string(&thread_id)
        .map_err(|_| ApiError::InvalidRequest("Invalid thread ID".to_string()))?;

    // Check if thread is already active
    if state.thread_manager.get_thread(thread_id).await.is_ok() {
        // Thread is already active, return success (idempotent)
        return Ok(Json(ResumeThreadResponse {
            success: true,
            thread_id: thread_id.to_string(),
        }));
    }

    // Load config (could support overrides in future)
    let config = Config::load_with_cli_overrides(vec![])
        .await
        .map_err(|e| ApiError::InternalError(format!("Failed to load config: {e}")))?;

    // Prefer Postgres-backed rollouts when configured.
    let postgres_enabled = std::env::var("CODEX_ROLLOUT_POSTGRES_URL")
        .ok()
        .is_some_and(|value| !value.trim().is_empty());

    let new_thread = if postgres_enabled {
        state
            .thread_manager
            .resume_thread_from_postgres(config, thread_id, state.auth_manager.clone())
            .await
            .map_err(|err| match err {
                CodexErr::Io(io) if io.kind() == ErrorKind::NotFound => {
                    ApiError::NotFound(format!("Rollout history not found for thread: {thread_id}"))
                }
                CodexErr::ThreadNotFound(_) => {
                    ApiError::NotFound(format!("Rollout history not found for thread: {thread_id}"))
                }
                other => ApiError::InternalError(format!("Failed to resume thread: {other}")),
            })?
    } else {
        let Some(rollout_path) =
            codex_core::find_thread_path_by_id_str(&state.codex_home, &thread_id.to_string())
                .await
                .map_err(|e| ApiError::InternalError(format!("Failed to locate rollout: {e}")))?
        else {
            return Err(ApiError::NotFound(format!(
                "Rollout file not found for thread: {thread_id}"
            )));
        };
        state
            .thread_manager
            .resume_thread_from_rollout(config, rollout_path, state.auth_manager.clone())
            .await
            .map_err(|e| ApiError::InternalError(format!("Failed to resume thread: {e}")))?
    };

    Ok(Json(ResumeThreadResponse {
        success: true,
        thread_id: new_thread.thread_id.to_string(),
    }))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ForkThreadRequest {
    pub turn_id: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ForkThreadResponse {
    pub new_thread_id: String,
    pub source_thread_id: String,
}

/// POST /api/v2/threads/:id/fork
///
/// Forks a thread from a specific turn (or latest turn if not specified)
#[utoipa::path(
    post,
    path = "/api/v2/threads/{id}/fork",
    params(
        ("id" = String, Path, description = "Source thread ID")
    ),
    request_body = ForkThreadRequest,
    responses(
        (status = 200, description = "Thread forked successfully", body = ForkThreadResponse),
        (status = 400, description = "Invalid request"),
        (status = 404, description = "Thread not found"),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Threads"
)]
pub async fn fork_thread(
    State(state): State<WebServerState>,
    Path(thread_id): Path<String>,
    Json(req): Json<ForkThreadRequest>,
) -> Result<Json<ForkThreadResponse>, ApiError> {
    let source_thread_id = ThreadId::from_string(&thread_id)
        .map_err(|_| ApiError::InvalidRequest("Invalid thread ID".to_string()))?;
    let _turn_id = req.turn_id;

    // Get rollout path for the source thread
    // Load config (TODO: support config overrides from request)
    let config = Config::load_with_cli_overrides(vec![])
        .await
        .map_err(|e| ApiError::InternalError(format!("Failed to load config: {e}")))?;

    // Prefer Postgres-backed rollouts when configured.
    let postgres_enabled = std::env::var("CODEX_ROLLOUT_POSTGRES_URL")
        .ok()
        .is_some_and(|value| !value.trim().is_empty());

    // Fork the thread (usize::MAX keeps full history, matching app-server behavior)
    // NOTE: turn_id is currently ignored - app-server doesn't support partial forks via JSON-RPC
    let new_thread = if postgres_enabled {
        state
            .thread_manager
            .fork_thread_from_postgres(usize::MAX, config, source_thread_id)
            .await
            .map_err(|err| match err {
                CodexErr::Io(io) if io.kind() == ErrorKind::NotFound => ApiError::ThreadNotFound,
                CodexErr::ThreadNotFound(_) => ApiError::ThreadNotFound,
                other => ApiError::InternalError(format!("Failed to fork thread: {other}")),
            })?
    } else {
        let source_thread = state
            .thread_manager
            .get_thread(source_thread_id)
            .await
            .map_err(|_| ApiError::ThreadNotFound)?;
        let rollout_path = source_thread.rollout_path().ok_or_else(|| {
            ApiError::InvalidRequest("Source thread has no rollout path".to_string())
        })?;
        state
            .thread_manager
            .fork_thread(usize::MAX, config, rollout_path)
            .await
            .map_err(|e| ApiError::InternalError(format!("Failed to fork thread: {e}")))?
    };

    let new_thread_id = new_thread.thread_id;

    Ok(Json(ForkThreadResponse {
        new_thread_id: new_thread_id.to_string(),
        source_thread_id: source_thread_id.to_string(),
    }))
}
