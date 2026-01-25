use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use serde::Deserialize;
use serde::Serialize;
use utoipa::ToSchema;

use crate::approval_manager::ApprovalManager;
use crate::error::ApiError;
use crate::state::ApprovalDecision;
use crate::state::WebServerState;

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalRequest {
    pub decision: ApprovalDecision,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ApprovalResponse {
    pub success: bool,
}

#[utoipa::path(
    post,
    path = "/api/v2/threads/{thread_id}/approvals/{approval_id}",
    request_body = ApprovalRequest,
    params(
        ("thread_id" = String, Path, description = "Thread ID"),
        ("approval_id" = String, Path, description = "Approval request ID (usually item_id)")
    ),
    responses(
        (status = 200, description = "Approval response submitted successfully", body = ApprovalResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Approval request not found"),
        (status = 408, description = "Approval request timed out"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Approvals"
)]
pub async fn respond_to_approval(
    State(state): State<WebServerState>,
    Path((thread_id, approval_id)): Path<(String, String)>,
    Json(req): Json<ApprovalRequest>,
) -> Result<Json<ApprovalResponse>, ApiError> {
    // Validate thread_id
    let _thread_id = codex_protocol::ThreadId::from_string(&thread_id)
        .map_err(|_| ApiError::InvalidRequest("Invalid thread ID".to_string()))?;

    // Create approval manager
    let approval_manager = ApprovalManager::new(state.pending_approvals.clone());

    // Respond to approval
    approval_manager
        .respond_to_approval(&approval_id, req.decision)
        .await
        .map_err(|e| {
            if e.contains("not found") {
                ApiError::InvalidRequest("Approval request not found".to_string())
            } else if e.contains("timed out") {
                ApiError::InvalidRequest("Approval request has timed out".to_string())
            } else {
                ApiError::InternalError(e)
            }
        })?;

    Ok(Json(ApprovalResponse { success: true }))
}
