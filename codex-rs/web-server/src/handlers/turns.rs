use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use serde::Deserialize;
use serde::Serialize;
use utoipa::ToSchema;

use crate::error::ApiError;
use crate::state::WebServerState;

#[derive(Debug, Deserialize, ToSchema)]
pub struct SendTurnRequest {
    pub input: Vec<UserInputItem>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(tag = "type")]
pub enum UserInputItem {
    #[serde(rename = "text")]
    Text {
        #[schema(example = "Hello, Codex!")]
        text: String,
    },
    #[serde(rename = "attachment")]
    Attachment {
        #[schema(example = "019bcfb9-4ea6-72e0-b43d-6b7e26ff0daf")]
        attachment_id: String,
    },
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SendTurnResponse {
    #[schema(example = "turn-12345")]
    pub turn_id: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct InterruptTurnRequest {}

#[derive(Debug, Serialize, ToSchema)]
pub struct InterruptTurnResponse {
    pub success: bool,
}

#[utoipa::path(
    post,
    path = "/api/v2/threads/{thread_id}/turns",
    request_body = SendTurnRequest,
    params(
        ("thread_id" = String, Path, description = "Thread ID")
    ),
    responses(
        (status = 200, description = "Turn submitted successfully", body = SendTurnResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Thread not found"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Turns"
)]
pub async fn send_turn(
    State(state): State<WebServerState>,
    Path(thread_id): Path<String>,
    Json(req): Json<SendTurnRequest>,
) -> Result<Json<SendTurnResponse>, ApiError> {
    let thread_id = codex_protocol::ThreadId::from_string(&thread_id)
        .map_err(|_| ApiError::InvalidRequest("Invalid thread ID".to_string()))?;

    let thread = state
        .thread_manager
        .get_thread(thread_id)
        .await
        .map_err(|_| ApiError::ThreadNotFound)?;

    let mut user_inputs = Vec::new();

    for item in req.input {
        match item {
            UserInputItem::Text { text } => {
                user_inputs.push(UserInput::Text {
                    text,
                    text_elements: Vec::new(),
                });
            }
            UserInputItem::Attachment { attachment_id } => {
                uuid::Uuid::parse_str(&attachment_id).map_err(|_| {
                    ApiError::InvalidRequest("Invalid attachment ID format".to_string())
                })?;

                let attachment_path = state.attachments_dir.join(&attachment_id);
                if !attachment_path.exists() {
                    return Err(ApiError::AttachmentNotFound);
                }

                let canonical_path = attachment_path
                    .canonicalize()
                    .map_err(|_| ApiError::AttachmentNotFound)?;
                let canonical_attachments_dir =
                    state.attachments_dir.canonicalize().map_err(|e| {
                        ApiError::InternalError(format!(
                            "Failed to resolve attachments directory: {e}"
                        ))
                    })?;

                if !canonical_path.starts_with(&canonical_attachments_dir) {
                    return Err(ApiError::InvalidRequest(
                        "Invalid attachment path".to_string(),
                    ));
                }

                user_inputs.push(UserInput::LocalImage {
                    path: canonical_path,
                });
            }
        }
    }

    let turn_id: String = thread
        .submit(Op::UserInput {
            items: user_inputs,
            final_output_json_schema: None,
        })
        .await
        .map_err(|e| ApiError::InternalError(format!("Failed to submit turn: {e}")))?;

    Ok(Json(SendTurnResponse { turn_id }))
}

#[utoipa::path(
    post,
    path = "/api/v2/threads/{thread_id}/turns/interrupt",
    request_body = InterruptTurnRequest,
    params(
        ("thread_id" = String, Path, description = "Thread ID")
    ),
    responses(
        (status = 200, description = "Turn interrupted successfully", body = InterruptTurnResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Thread not found"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Turns"
)]
pub async fn interrupt_turn(
    State(state): State<WebServerState>,
    Path(thread_id): Path<String>,
    Json(_req): Json<InterruptTurnRequest>,
) -> Result<Json<InterruptTurnResponse>, ApiError> {
    let thread_id = codex_protocol::ThreadId::from_string(&thread_id)
        .map_err(|_| ApiError::InvalidRequest("Invalid thread ID".to_string()))?;

    let thread = state
        .thread_manager
        .get_thread(thread_id)
        .await
        .map_err(|_| ApiError::ThreadNotFound)?;

    thread
        .submit(Op::Interrupt)
        .await
        .map_err(|e| ApiError::InternalError(format!("Failed to interrupt turn: {e}")))?;

    Ok(Json(InterruptTurnResponse { success: true }))
}
