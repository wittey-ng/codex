pub mod approvals;
pub mod auth;
pub mod commands;
pub mod config;
pub mod feedback;
pub mod mcp;
pub mod models;
pub mod review;
pub mod skills;
pub mod threads;
pub mod turns;

use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use axum::response::sse::Event;
use axum::response::sse::Sse;
use codex_core::config::Config;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use futures::stream::Stream;
use serde::Deserialize;
use serde::Serialize;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;
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
    #[schema(example = "019bcfb9-4ea6-72e0-b43d-6b7e26ff0daf")]
    pub thread_id: String,
    #[schema(example = "claude-sonnet-4-5")]
    pub model: String,
}

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

#[utoipa::path(
    post,
    path = "/api/v1/threads",
    request_body = CreateThreadRequest,
    responses(
        (status = 200, description = "Thread created successfully", body = CreateThreadResponse),
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

#[utoipa::path(
    post,
    path = "/api/v1/threads/{thread_id}/turns",
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
    get,
    path = "/api/v1/threads/{thread_id}/events",
    params(
        ("thread_id" = String, Path, description = "Thread ID")
    ),
    responses(
        (status = 200, description = "SSE event stream", content_type = "text/event-stream"),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Thread not found")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Events"
)]
pub async fn stream_events(
    State(state): State<WebServerState>,
    Path(thread_id): Path<String>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
    use crate::event_stream::EventStreamProcessor;
    use crate::state::ApprovalContext;
    use codex_app_server_protocol::CommandExecutionRequestApprovalParams;
    use codex_app_server_protocol::FileChangeRequestApprovalParams;
    use codex_protocol::protocol::EventMsg;
    use codex_protocol::protocol::Op;
    use codex_protocol::protocol::ReviewDecision;
    use tokio::sync::oneshot;

    let thread_id = codex_protocol::ThreadId::from_string(&thread_id)
        .map_err(|_| ApiError::InvalidRequest("Invalid thread ID".to_string()))?;

    let thread = state
        .thread_manager
        .get_thread(thread_id)
        .await
        .map_err(|_| ApiError::ThreadNotFound)?;

    // Register stream in session store
    {
        let mut sessions = state.sessions.write().await;
        sessions.register_stream(thread_id);
    }

    let event_processor = EventStreamProcessor::new(thread_id, Arc::new(state.clone()));
    let state_for_stream = state.clone();
    let thread_for_approval = thread.clone();

    let stream = async_stream::stream! {
        loop {
            match thread.next_event().await {
                Ok(event) => {
                    let event_msg = event.msg.clone();

                    // Special handling for approval requests
                    match &event_msg {
                        EventMsg::ExecApprovalRequest(ev) => {
                            // Register approval context
                            let (tx, rx) = oneshot::channel();
                            let approval_id = ev.call_id.clone();
                            let approval_ctx = ApprovalContext {
                                thread_id,
                                item_id: approval_id.clone(),
                                approval_type: crate::state::ApprovalType::CommandExecution {
                                    command: ev.command.clone(),
                                    cwd: ev.cwd.clone(),
                                    reason: ev.reason.clone().unwrap_or_default(),
                                },
                                response_channel: tx,
                                created_at: std::time::Instant::now(),
                                timeout: Duration::from_secs(900), // 15 minutes
                            };

                            {
                                let mut approvals = state_for_stream.pending_approvals.lock().await;
                                approvals.insert(approval_id.clone(), approval_ctx);
                            }

                            // Send approval request as SSE event
                            let params = CommandExecutionRequestApprovalParams {
                                thread_id: thread_id.to_string(),
                                turn_id: ev.turn_id.clone(),
                                item_id: approval_id.clone(),
                                reason: ev.reason.clone(),
                                command: Some(ev.command.join(" ")),
                                cwd: Some(ev.cwd.clone()),
                                command_actions: None,
                                proposed_execpolicy_amendment: ev.proposed_execpolicy_amendment.clone().map(std::convert::Into::into),
                            };

                            let event_type = "item/commandExecution/requestApproval";
                            let json_data = serde_json::to_string(&params).unwrap_or_default();
                            yield Ok(Event::default().event(event_type).data(json_data));

                            // Spawn task to wait for approval response
                            let thread_clone = thread_for_approval.clone();
                            let turn_id_clone = ev.turn_id.clone();
                            tokio::spawn(async move {
                                match rx.await {
                                    Ok(response) => {
                                        let decision = match response.decision {
                                            crate::state::ApprovalDecision::Approve => {
                                                ReviewDecision::Approved
                                            }
                                            crate::state::ApprovalDecision::Decline => {
                                                ReviewDecision::Denied
                                            }
                                        };

                                        if let Err(e) = thread_clone
                                            .submit(Op::ExecApproval {
                                                id: turn_id_clone,
                                                decision,
                                            })
                                            .await
                                        {
                                            tracing::error!("Failed to submit exec approval: {}", e);
                                        }
                                    }
                                    Err(_) => {
                                        // Channel closed, submit denial
                                        if let Err(e) = thread_clone
                                            .submit(Op::ExecApproval {
                                                id: turn_id_clone,
                                                decision: ReviewDecision::Denied,
                                            })
                                            .await
                                        {
                                            tracing::error!("Failed to submit denied exec approval: {}", e);
                                        }
                                    }
                                }
                            });
                        }

                        EventMsg::ApplyPatchApprovalRequest(ev) => {
                            // Register approval context
                            let (tx, rx) = oneshot::channel();
                            let approval_id = ev.call_id.clone();
                            let approval_ctx = ApprovalContext {
                                thread_id,
                                item_id: approval_id.clone(),
                                approval_type: crate::state::ApprovalType::FileChange {
                                    reason: ev.reason.clone().unwrap_or_default(),
                                },
                                response_channel: tx,
                                created_at: std::time::Instant::now(),
                                timeout: Duration::from_secs(900), // 15 minutes
                            };

                            {
                                let mut approvals = state_for_stream.pending_approvals.lock().await;
                                approvals.insert(approval_id.clone(), approval_ctx);
                            }

                            // Send approval request as SSE event
                            let params = FileChangeRequestApprovalParams {
                                thread_id: thread_id.to_string(),
                                turn_id: ev.turn_id.clone(),
                                item_id: approval_id.clone(),
                                reason: ev.reason.clone(),
                                grant_root: ev.grant_root.clone(),
                            };

                            let event_type = "item/fileChange/requestApproval";
                            let json_data = serde_json::to_string(&params).unwrap_or_default();
                            yield Ok(Event::default().event(event_type).data(json_data));

                            // Spawn task to wait for approval response
                            let thread_clone = thread_for_approval.clone();
                            let turn_id_clone = ev.turn_id.clone();
                            tokio::spawn(async move {
                                match rx.await {
                                    Ok(response) => {
                                        let decision = match response.decision {
                                            crate::state::ApprovalDecision::Approve => {
                                                ReviewDecision::Approved
                                            }
                                            crate::state::ApprovalDecision::Decline => {
                                                ReviewDecision::Denied
                                            }
                                        };

                                        if let Err(e) = thread_clone
                                            .submit(Op::PatchApproval {
                                                id: turn_id_clone,
                                                decision,
                                            })
                                            .await
                                        {
                                            tracing::error!("Failed to submit patch approval: {}", e);
                                        }
                                    }
                                    Err(_) => {
                                        // Channel closed, submit denial
                                        if let Err(e) = thread_clone
                                            .submit(Op::PatchApproval {
                                                id: turn_id_clone,
                                                decision: ReviewDecision::Denied,
                                            })
                                            .await
                                        {
                                            tracing::error!("Failed to submit denied patch approval: {}", e);
                                        }
                                    }
                                }
                            });
                        }

                        _ => {
                            // Process all other events through EventStreamProcessor
                            let notifications = event_processor.process_event(event).await;

                            for notification in notifications {
                                let event_type = EventStreamProcessor::event_type_name(&notification);
                                let json_data = serde_json::to_string(&notification).unwrap_or_default();

                                yield Ok(Event::default()
                                    .event(event_type)
                                    .data(json_data));
                            }
                        }
                    }
                }
                Err(_) => {
                    // Unregister stream on error/completion
                    let mut sessions = state_for_stream.sessions.write().await;
                    sessions.unregister_stream(thread_id);
                    break;
                }
            }
        }
    };

    Ok(Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(10))
            .text("keepalive"),
    ))
}
