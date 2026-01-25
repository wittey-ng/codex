use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use axum::http::StatusCode;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ReviewRequest as CoreReviewRequest;
use codex_protocol::protocol::ReviewTarget as CoreReviewTarget;
use serde::Deserialize;
use serde::Serialize;
use std::result::Result;
use utoipa::ToSchema;

use crate::error::ApiError;
use crate::state::WebServerState;

#[derive(Debug, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ReviewTarget {
    Git {
        #[allow(dead_code)]
        branch: Option<String>,
        base: Option<String>,
    },
    Files {
        paths: Vec<String>,
    },
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ReviewDelivery {
    Inline,
    Detached,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct StartReviewRequest {
    pub target: ReviewTarget,
    #[serde(default)]
    pub delivery: Option<ReviewDelivery>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct StartReviewResponse {
    pub review_id: String,
    pub thread_id: String,
}

/// POST /api/v2/threads/:id/reviews
///
/// Starts a code review in inline mode (within existing thread)
#[utoipa::path(
    post,
    path = "/api/v2/threads/{id}/reviews",
    params(
        ("id" = String, Path, description = "Thread ID")
    ),
    request_body = StartReviewRequest,
    responses(
        (status = 202, description = "Review started (streaming via SSE)", body = StartReviewResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Thread not found"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Review"
)]
pub async fn start_inline_review(
    State(state): State<WebServerState>,
    Path(thread_id): Path<String>,
    Json(req): Json<StartReviewRequest>,
) -> Result<(StatusCode, Json<StartReviewResponse>), ApiError> {
    let thread_id = codex_protocol::ThreadId::from_string(&thread_id)
        .map_err(|_| ApiError::InvalidRequest("Invalid thread ID".to_string()))?;

    let thread = state
        .thread_manager
        .get_thread(thread_id)
        .await
        .map_err(|_| ApiError::ThreadNotFound)?;

    // Convert ReviewTarget to CoreReviewTarget
    let StartReviewRequest { target, delivery } = req;
    let _delivery = delivery;
    let review_request = build_review_request(target)?;

    // Submit Op::Review
    let turn_id = thread
        .submit(Op::Review { review_request })
        .await
        .map_err(|e| ApiError::InternalError(format!("Failed to start review: {e}")))?;

    // Review will stream via SSE
    Ok((
        StatusCode::ACCEPTED,
        Json(StartReviewResponse {
            review_id: turn_id,
            thread_id: thread_id.to_string(),
        }),
    ))
}

/// POST /api/v2/reviews
///
/// Starts a code review in detached mode (creates new thread)
#[utoipa::path(
    post,
    path = "/api/v2/reviews",
    request_body = StartReviewRequest,
    responses(
        (status = 202, description = "Review started (streaming via SSE)", body = StartReviewResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Review"
)]
pub async fn start_detached_review(
    State(state): State<WebServerState>,
    Json(req): Json<StartReviewRequest>,
) -> Result<(StatusCode, Json<StartReviewResponse>), ApiError> {
    // Load config
    let config = codex_core::config::Config::load_with_cli_overrides(vec![])
        .await
        .map_err(|e| ApiError::InternalError(format!("Failed to load config: {e}")))?;

    // Start new thread for detached review
    let new_thread = state
        .thread_manager
        .start_thread(config)
        .await
        .map_err(|e| ApiError::InternalError(format!("Failed to start review thread: {e}")))?;

    let thread_id = new_thread.thread_id;

    // Get the thread to submit the review request
    let thread = state
        .thread_manager
        .get_thread(thread_id)
        .await
        .map_err(|_| ApiError::InternalError("Failed to get created thread".to_string()))?;

    // Convert ReviewTarget to CoreReviewRequest
    let StartReviewRequest { target, delivery } = req;
    let _delivery = delivery;
    let review_request = build_review_request(target)?;

    // Submit Op::Review
    let turn_id = thread
        .submit(Op::Review { review_request })
        .await
        .map_err(|e| {
            ApiError::InternalError(format!("Failed to start detached review turn: {e}"))
        })?;

    // Review will stream via SSE
    Ok((
        StatusCode::ACCEPTED,
        Json(StartReviewResponse {
            review_id: turn_id,
            thread_id: thread_id.to_string(),
        }),
    ))
}

// Helper function to convert API ReviewTarget to Core ReviewRequest
fn build_review_request(target: ReviewTarget) -> Result<CoreReviewRequest, ApiError> {
    let core_target = match target {
        ReviewTarget::Git { base, .. } => CoreReviewTarget::BaseBranch {
            branch: base.unwrap_or_else(|| "main".to_string()),
        },
        ReviewTarget::Files { paths } => {
            // Convert file paths to Custom instructions
            let instructions = format!("Review the following files: {}", paths.join(", "));
            CoreReviewTarget::Custom { instructions }
        }
    };

    Ok(CoreReviewRequest {
        target: core_target,
        user_facing_hint: None,
    })
}
