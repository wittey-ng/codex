use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use codex_protocol::ThreadId;
use serde::Deserialize;
use serde::Serialize;
use std::result::Result;
use utoipa::ToSchema;

use crate::error::ApiError;
use crate::state::WebServerState;

#[derive(Debug, Deserialize, ToSchema)]
pub struct UploadFeedbackRequest {
    pub classification: String, // e.g., "bug", "bad_result", "good_result"
    pub reason: Option<String>,
    pub thread_id: Option<String>,
    #[serde(default)]
    pub include_logs: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UploadFeedbackResponse {
    pub success: bool,
    pub thread_id: String,
}

/// POST /api/v2/feedback
///
/// Uploads user feedback (fire-and-forget)
#[utoipa::path(
    post,
    path = "/api/v2/feedback",
    request_body = UploadFeedbackRequest,
    responses(
        (status = 201, description = "Feedback uploaded successfully", body = UploadFeedbackResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Feedback"
)]
pub async fn upload_feedback(
    State(state): State<WebServerState>,
    Json(req): Json<UploadFeedbackRequest>,
) -> Result<(StatusCode, Json<UploadFeedbackResponse>), ApiError> {
    // Validate classification
    if req.classification.is_empty() {
        return Err(ApiError::InvalidRequest(
            "Classification cannot be empty".to_string(),
        ));
    }

    // Resolve thread_id and rollout_path
    let (thread_id, rollout_path) = if let Some(tid_str) = &req.thread_id {
        let tid = ThreadId::from_string(tid_str)
            .map_err(|_| ApiError::InvalidRequest("Invalid thread ID".to_string()))?;

        // Try to get rollout path from active thread
        let path = state
            .thread_manager
            .get_thread(tid)
            .await
            .ok()
            .and_then(|thread| thread.rollout_path());

        (tid_str.clone(), path)
    } else {
        // Generate a unique thread_id for tracking
        let tid = ThreadId::new();
        (tid.to_string(), None)
    };

    // Create snapshot and upload in blocking task
    let feedback = state.feedback.clone();
    let classification = req.classification.clone();
    let classification_for_log = classification.clone();
    let reason = req.reason.clone();
    let include_logs = req.include_logs;
    let session_source = state.thread_manager.session_source();
    let thread_id_for_log = thread_id.clone();

    let upload_result = tokio::task::spawn_blocking(move || {
        let snapshot = feedback.snapshot(None);
        snapshot.upload_feedback(
            &classification,
            reason.as_deref(),
            include_logs,
            rollout_path.as_deref(),
            Some(session_source),
        )
    })
    .await
    .map_err(|e| ApiError::InternalError(format!("Failed to spawn feedback task: {e}")))?;

    match upload_result {
        Ok(()) => {
            tracing::info!(
                "Feedback uploaded successfully: classification={}, thread_id={}",
                classification_for_log,
                thread_id_for_log
            );
            Ok((
                StatusCode::CREATED,
                Json(UploadFeedbackResponse {
                    success: true,
                    thread_id,
                }),
            ))
        }
        Err(err) => {
            tracing::error!("Failed to upload feedback: {}", err);
            Err(ApiError::InternalError(format!(
                "Failed to upload feedback: {err}"
            )))
        }
    }
}
