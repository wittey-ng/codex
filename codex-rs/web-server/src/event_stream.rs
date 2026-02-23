use codex_app_server_protocol::*;
use codex_protocol::ThreadId;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use std::sync::Arc;

use crate::state::WebServerState;

/// Helper function to convert protocol FileChange to app-server FileUpdateChange
fn convert_file_change(
    path: std::path::PathBuf,
    change: codex_protocol::protocol::FileChange,
) -> FileUpdateChange {
    use codex_protocol::protocol::FileChange as CoreFileChange;

    match change {
        CoreFileChange::Add { content } => FileUpdateChange {
            path: path.to_string_lossy().into_owned(),
            kind: PatchChangeKind::Add,
            diff: content,
        },
        CoreFileChange::Delete { content } => FileUpdateChange {
            path: path.to_string_lossy().into_owned(),
            kind: PatchChangeKind::Delete,
            diff: content,
        },
        CoreFileChange::Update {
            unified_diff,
            move_path,
        } => FileUpdateChange {
            path: path.to_string_lossy().into_owned(),
            kind: PatchChangeKind::Update { move_path },
            diff: unified_diff,
        },
    }
}

pub struct EventStreamProcessor {
    thread_id: ThreadId,
    _state: Arc<WebServerState>,
}

impl EventStreamProcessor {
    pub fn new(thread_id: ThreadId, state: Arc<WebServerState>) -> Self {
        Self {
            thread_id,
            _state: state,
        }
    }

    // TODO: Approval request handling needs special integration in stream_events handler
    //
    // EventMsg::ExecApprovalRequest and ApplyPatchApprovalRequest cannot be handled here
    // because they require:
    // 1. Registering approval in state.pending_approvals with oneshot channel
    // 2. Sending custom SSE notification to client
    // 3. Blocking turn execution until approval received via REST POST
    // 4. Submitting Op::ExecApproval{id, decision} or Op::PatchApproval{id, decision}
    //
    // This must be implemented in handlers/mod.rs::stream_events where we have
    // access to the thread and can spawn async tasks to wait for approval responses.
    //
    // Reference: app-server/src/bespoke_event_handling.rs:195-260

    pub async fn process_event(&self, event: Event) -> Vec<ServerNotification> {
        let Event { id: turn_id, msg } = event;

        match msg {
            EventMsg::ItemStarted(ev) => {
                vec![ServerNotification::ItemStarted(ItemStartedNotification {
                    thread_id: self.thread_id.to_string(),
                    turn_id,
                    item: ev.item.into(),
                })]
            }

            EventMsg::ItemCompleted(ev) => {
                vec![ServerNotification::ItemCompleted(
                    ItemCompletedNotification {
                        thread_id: self.thread_id.to_string(),
                        turn_id,
                        item: ev.item.into(),
                    },
                )]
            }

            EventMsg::AgentMessageContentDelta(ev) => {
                vec![ServerNotification::AgentMessageDelta(
                    AgentMessageDeltaNotification {
                        thread_id: self.thread_id.to_string(),
                        turn_id,
                        item_id: ev.item_id,
                        delta: ev.delta,
                    },
                )]
            }

            EventMsg::ExecCommandOutputDelta(ev) => {
                let delta = String::from_utf8_lossy(&ev.chunk).to_string();
                vec![ServerNotification::CommandExecutionOutputDelta(
                    CommandExecutionOutputDeltaNotification {
                        thread_id: self.thread_id.to_string(),
                        turn_id,
                        item_id: ev.call_id,
                        delta,
                    },
                )]
            }

            EventMsg::TerminalInteraction(ev) => {
                vec![ServerNotification::TerminalInteraction(
                    TerminalInteractionNotification {
                        thread_id: self.thread_id.to_string(),
                        turn_id,
                        item_id: ev.call_id,
                        process_id: ev.process_id,
                        stdin: ev.stdin,
                    },
                )]
            }

            EventMsg::ContextCompacted(_) => {
                vec![ServerNotification::ContextCompacted(
                    ContextCompactedNotification {
                        thread_id: self.thread_id.to_string(),
                        turn_id,
                    },
                )]
            }

            EventMsg::DeprecationNotice(ev) => {
                vec![ServerNotification::DeprecationNotice(
                    DeprecationNoticeNotification {
                        summary: ev.summary,
                        details: ev.details,
                    },
                )]
            }

            EventMsg::ReasoningContentDelta(ev) => {
                vec![ServerNotification::ReasoningSummaryTextDelta(
                    ReasoningSummaryTextDeltaNotification {
                        thread_id: self.thread_id.to_string(),
                        turn_id,
                        item_id: ev.item_id,
                        delta: ev.delta,
                        summary_index: ev.summary_index,
                    },
                )]
            }

            EventMsg::ReasoningRawContentDelta(ev) => {
                vec![ServerNotification::ReasoningTextDelta(
                    ReasoningTextDeltaNotification {
                        thread_id: self.thread_id.to_string(),
                        turn_id,
                        item_id: ev.item_id,
                        delta: ev.delta,
                        content_index: ev.content_index,
                    },
                )]
            }

            EventMsg::AgentReasoningSectionBreak(ev) => {
                vec![ServerNotification::ReasoningSummaryPartAdded(
                    ReasoningSummaryPartAddedNotification {
                        thread_id: self.thread_id.to_string(),
                        turn_id,
                        item_id: ev.item_id,
                        summary_index: ev.summary_index,
                    },
                )]
            }

            EventMsg::TokenCount(ev) => {
                let mut notifications = Vec::new();

                if let Some(info) = ev.info {
                    notifications.push(ServerNotification::ThreadTokenUsageUpdated(
                        ThreadTokenUsageUpdatedNotification {
                            thread_id: self.thread_id.to_string(),
                            turn_id,
                            token_usage: ThreadTokenUsage::from(info),
                        },
                    ));
                }

                notifications
            }

            EventMsg::Error(ev) => {
                if matches!(
                    ev.codex_error_info,
                    Some(codex_protocol::protocol::CodexErrorInfo::ThreadRollbackFailed)
                ) {
                    return vec![];
                }

                vec![ServerNotification::Error(ErrorNotification {
                    error: TurnError {
                        message: ev.message,
                        codex_error_info: ev
                            .codex_error_info
                            .map(codex_app_server_protocol::CodexErrorInfo::from),
                        additional_details: None,
                    },
                    will_retry: false,
                    thread_id: self.thread_id.to_string(),
                    turn_id,
                })]
            }

            EventMsg::StreamError(ev) => {
                vec![ServerNotification::Error(ErrorNotification {
                    error: TurnError {
                        message: ev.message,
                        codex_error_info: ev
                            .codex_error_info
                            .map(codex_app_server_protocol::CodexErrorInfo::from),
                        additional_details: ev.additional_details,
                    },
                    will_retry: true,
                    thread_id: self.thread_id.to_string(),
                    turn_id,
                })]
            }

            EventMsg::ThreadRolledBack(_) => vec![],

            EventMsg::TurnDiff(ev) => {
                vec![ServerNotification::TurnDiffUpdated(
                    TurnDiffUpdatedNotification {
                        thread_id: self.thread_id.to_string(),
                        turn_id,
                        diff: ev.unified_diff,
                    },
                )]
            }

            EventMsg::PlanUpdate(ev) => {
                vec![ServerNotification::TurnPlanUpdated(
                    TurnPlanUpdatedNotification {
                        thread_id: self.thread_id.to_string(),
                        turn_id,
                        explanation: ev.explanation,
                        plan: ev.plan.into_iter().map(std::convert::Into::into).collect(),
                    },
                )]
            }

            EventMsg::TurnComplete(_) => {
                vec![ServerNotification::TurnCompleted(
                    TurnCompletedNotification {
                        thread_id: self.thread_id.to_string(),
                        turn: Turn {
                            id: turn_id,
                            items: vec![],
                            error: None,
                            status: TurnStatus::Completed,
                        },
                    },
                )]
            }

            EventMsg::TurnAborted(ev) => {
                vec![ServerNotification::TurnCompleted(
                    TurnCompletedNotification {
                        thread_id: self.thread_id.to_string(),
                        turn: Turn {
                            id: turn_id,
                            items: vec![],
                            error: Some(TurnError {
                                message: format!("Turn interrupted: {:?}", ev.reason),
                                codex_error_info: None,
                                additional_details: None,
                            }),
                            status: TurnStatus::Interrupted,
                        },
                    },
                )]
            }

            EventMsg::ExecCommandBegin(ev) => {
                let item = ThreadItem::CommandExecution {
                    id: ev.call_id.clone(),
                    command: ev.command.join(" "),
                    cwd: ev.cwd,
                    process_id: ev.process_id,
                    status: CommandExecutionStatus::InProgress,
                    command_actions: ev
                        .parsed_cmd
                        .into_iter()
                        .map(std::convert::Into::into)
                        .collect(),
                    aggregated_output: None,
                    exit_code: None,
                    duration_ms: None,
                };
                vec![ServerNotification::ItemStarted(ItemStartedNotification {
                    thread_id: self.thread_id.to_string(),
                    turn_id,
                    item,
                })]
            }

            EventMsg::ExecCommandEnd(ev) => {
                let status = if ev.exit_code == 0 {
                    CommandExecutionStatus::Completed
                } else {
                    CommandExecutionStatus::Failed
                };
                let aggregated_output = if ev.aggregated_output.is_empty() {
                    None
                } else {
                    Some(ev.aggregated_output)
                };
                let duration_ms = i64::try_from(ev.duration.as_millis()).unwrap_or(i64::MAX);

                let item = ThreadItem::CommandExecution {
                    id: ev.call_id,
                    command: ev.command.join(" "),
                    cwd: ev.cwd,
                    process_id: ev.process_id,
                    status,
                    command_actions: ev
                        .parsed_cmd
                        .into_iter()
                        .map(std::convert::Into::into)
                        .collect(),
                    aggregated_output,
                    exit_code: Some(ev.exit_code),
                    duration_ms: Some(duration_ms),
                };
                vec![ServerNotification::ItemCompleted(
                    ItemCompletedNotification {
                        thread_id: self.thread_id.to_string(),
                        turn_id,
                        item,
                    },
                )]
            }

            EventMsg::PatchApplyBegin(ev) => {
                let item = ThreadItem::FileChange {
                    id: ev.call_id,
                    changes: ev
                        .changes
                        .into_iter()
                        .map(|(path, change)| convert_file_change(path, change))
                        .collect(),
                    status: PatchApplyStatus::InProgress,
                };
                vec![ServerNotification::ItemStarted(ItemStartedNotification {
                    thread_id: self.thread_id.to_string(),
                    turn_id,
                    item,
                })]
            }

            EventMsg::PatchApplyEnd(ev) => {
                let status = if ev.success {
                    PatchApplyStatus::Completed
                } else {
                    PatchApplyStatus::Failed
                };
                let item = ThreadItem::FileChange {
                    id: ev.call_id,
                    changes: ev
                        .changes
                        .into_iter()
                        .map(|(path, change)| convert_file_change(path, change))
                        .collect(),
                    status,
                };
                vec![ServerNotification::ItemCompleted(
                    ItemCompletedNotification {
                        thread_id: self.thread_id.to_string(),
                        turn_id,
                        item,
                    },
                )]
            }

            EventMsg::ViewImageToolCall(ev) => {
                let item = ThreadItem::ImageView {
                    id: ev.call_id.clone(),
                    path: ev.path.to_string_lossy().into_owned(),
                };
                vec![
                    ServerNotification::ItemStarted(ItemStartedNotification {
                        thread_id: self.thread_id.to_string(),
                        turn_id: turn_id.clone(),
                        item: item.clone(),
                    }),
                    ServerNotification::ItemCompleted(ItemCompletedNotification {
                        thread_id: self.thread_id.to_string(),
                        turn_id,
                        item,
                    }),
                ]
            }

            EventMsg::RawResponseItem(ev) => {
                vec![ServerNotification::RawResponseItemCompleted(
                    RawResponseItemCompletedNotification {
                        thread_id: self.thread_id.to_string(),
                        turn_id,
                        item: ev.item,
                    },
                )]
            }

            EventMsg::McpToolCallBegin(ev) => {
                let item = ThreadItem::McpToolCall {
                    id: ev.call_id,
                    server: ev.invocation.server,
                    tool: ev.invocation.tool,
                    status: McpToolCallStatus::InProgress,
                    arguments: ev.invocation.arguments.unwrap_or(serde_json::Value::Null),
                    result: None,
                    error: None,
                    duration_ms: None,
                };
                vec![ServerNotification::ItemStarted(ItemStartedNotification {
                    thread_id: self.thread_id.to_string(),
                    turn_id,
                    item,
                })]
            }

            EventMsg::McpToolCallEnd(ev) => {
                let (status, result, error) = match ev.result {
                    Ok(call_result) => {
                        let mcp_result = McpToolCallResult {
                            content: call_result.content.into_iter().collect(),
                            structured_content: None,
                        };
                        (McpToolCallStatus::Completed, Some(mcp_result), None)
                    }
                    Err(err_msg) => {
                        let mcp_error = McpToolCallError { message: err_msg };
                        (McpToolCallStatus::Failed, None, Some(mcp_error))
                    }
                };
                let duration_ms = i64::try_from(ev.duration.as_millis()).unwrap_or(i64::MAX);

                let item = ThreadItem::McpToolCall {
                    id: ev.call_id,
                    server: ev.invocation.server,
                    tool: ev.invocation.tool,
                    status,
                    arguments: ev.invocation.arguments.unwrap_or(serde_json::Value::Null),
                    result,
                    error,
                    duration_ms: Some(duration_ms),
                };
                vec![ServerNotification::ItemCompleted(
                    ItemCompletedNotification {
                        thread_id: self.thread_id.to_string(),
                        turn_id,
                        item,
                    },
                )]
            }

            EventMsg::CollabAgentSpawnBegin(ev) => {
                let item = ThreadItem::CollabAgentToolCall {
                    id: ev.call_id,
                    tool: CollabAgentTool::SpawnAgent,
                    status: CollabAgentToolCallStatus::InProgress,
                    sender_thread_id: ev.sender_thread_id.to_string(),
                    receiver_thread_ids: Vec::new(),
                    prompt: Some(ev.prompt),
                    agents_states: std::collections::HashMap::new(),
                };
                vec![ServerNotification::ItemStarted(ItemStartedNotification {
                    thread_id: self.thread_id.to_string(),
                    turn_id,
                    item,
                })]
            }

            EventMsg::CollabAgentSpawnEnd(ev) => {
                let has_receiver = ev.new_thread_id.is_some();
                let status = match &ev.status {
                    codex_protocol::protocol::AgentStatus::Errored(_)
                    | codex_protocol::protocol::AgentStatus::NotFound => {
                        CollabAgentToolCallStatus::Failed
                    }
                    _ if has_receiver => CollabAgentToolCallStatus::Completed,
                    _ => CollabAgentToolCallStatus::Failed,
                };
                let (receiver_thread_ids, agents_states) = match ev.new_thread_id {
                    Some(id) => {
                        let receiver_id = id.to_string();
                        let received_state = CollabAgentState::from(ev.status.clone());
                        (
                            vec![receiver_id.clone()],
                            [(receiver_id, received_state)].into_iter().collect(),
                        )
                    }
                    None => (Vec::new(), std::collections::HashMap::new()),
                };
                let item = ThreadItem::CollabAgentToolCall {
                    id: ev.call_id,
                    tool: CollabAgentTool::SpawnAgent,
                    status,
                    sender_thread_id: ev.sender_thread_id.to_string(),
                    receiver_thread_ids,
                    prompt: Some(ev.prompt),
                    agents_states,
                };
                vec![ServerNotification::ItemCompleted(
                    ItemCompletedNotification {
                        thread_id: self.thread_id.to_string(),
                        turn_id,
                        item,
                    },
                )]
            }

            EventMsg::CollabAgentInteractionBegin(ev) => {
                let receiver_thread_ids = vec![ev.receiver_thread_id.to_string()];
                let item = ThreadItem::CollabAgentToolCall {
                    id: ev.call_id,
                    tool: CollabAgentTool::SendInput,
                    status: CollabAgentToolCallStatus::InProgress,
                    sender_thread_id: ev.sender_thread_id.to_string(),
                    receiver_thread_ids,
                    prompt: Some(ev.prompt),
                    agents_states: std::collections::HashMap::new(),
                };
                vec![ServerNotification::ItemStarted(ItemStartedNotification {
                    thread_id: self.thread_id.to_string(),
                    turn_id,
                    item,
                })]
            }

            EventMsg::CollabAgentInteractionEnd(ev) => {
                let status = match &ev.status {
                    codex_protocol::protocol::AgentStatus::Errored(_)
                    | codex_protocol::protocol::AgentStatus::NotFound => {
                        CollabAgentToolCallStatus::Failed
                    }
                    _ => CollabAgentToolCallStatus::Completed,
                };
                let receiver_id = ev.receiver_thread_id.to_string();
                let received_state = CollabAgentState::from(ev.status);
                let item = ThreadItem::CollabAgentToolCall {
                    id: ev.call_id,
                    tool: CollabAgentTool::SendInput,
                    status,
                    sender_thread_id: ev.sender_thread_id.to_string(),
                    receiver_thread_ids: vec![receiver_id.clone()],
                    prompt: Some(ev.prompt),
                    agents_states: [(receiver_id, received_state)].into_iter().collect(),
                };
                vec![ServerNotification::ItemCompleted(
                    ItemCompletedNotification {
                        thread_id: self.thread_id.to_string(),
                        turn_id,
                        item,
                    },
                )]
            }

            EventMsg::CollabWaitingBegin(ev) => {
                let receiver_thread_ids = ev
                    .receiver_thread_ids
                    .iter()
                    .map(ToString::to_string)
                    .collect();
                let item = ThreadItem::CollabAgentToolCall {
                    id: ev.call_id,
                    tool: CollabAgentTool::Wait,
                    status: CollabAgentToolCallStatus::InProgress,
                    sender_thread_id: ev.sender_thread_id.to_string(),
                    receiver_thread_ids,
                    prompt: None,
                    agents_states: std::collections::HashMap::new(),
                };
                vec![ServerNotification::ItemStarted(ItemStartedNotification {
                    thread_id: self.thread_id.to_string(),
                    turn_id,
                    item,
                })]
            }

            EventMsg::CollabWaitingEnd(ev) => {
                let status = if ev.statuses.values().any(|status| {
                    matches!(
                        status,
                        codex_protocol::protocol::AgentStatus::Errored(_)
                            | codex_protocol::protocol::AgentStatus::NotFound
                    )
                }) {
                    CollabAgentToolCallStatus::Failed
                } else {
                    CollabAgentToolCallStatus::Completed
                };
                let receiver_thread_ids = ev.statuses.keys().map(ToString::to_string).collect();
                let agents_states = ev
                    .statuses
                    .iter()
                    .map(|(id, status)| (id.to_string(), CollabAgentState::from(status.clone())))
                    .collect();
                let item = ThreadItem::CollabAgentToolCall {
                    id: ev.call_id,
                    tool: CollabAgentTool::Wait,
                    status,
                    sender_thread_id: ev.sender_thread_id.to_string(),
                    receiver_thread_ids,
                    prompt: None,
                    agents_states,
                };
                vec![ServerNotification::ItemCompleted(
                    ItemCompletedNotification {
                        thread_id: self.thread_id.to_string(),
                        turn_id,
                        item,
                    },
                )]
            }

            EventMsg::CollabCloseBegin(ev) => {
                let item = ThreadItem::CollabAgentToolCall {
                    id: ev.call_id,
                    tool: CollabAgentTool::CloseAgent,
                    status: CollabAgentToolCallStatus::InProgress,
                    sender_thread_id: ev.sender_thread_id.to_string(),
                    receiver_thread_ids: vec![ev.receiver_thread_id.to_string()],
                    prompt: None,
                    agents_states: std::collections::HashMap::new(),
                };
                vec![ServerNotification::ItemStarted(ItemStartedNotification {
                    thread_id: self.thread_id.to_string(),
                    turn_id,
                    item,
                })]
            }

            EventMsg::CollabCloseEnd(ev) => {
                let status = match &ev.status {
                    codex_protocol::protocol::AgentStatus::Errored(_)
                    | codex_protocol::protocol::AgentStatus::NotFound => {
                        CollabAgentToolCallStatus::Failed
                    }
                    _ => CollabAgentToolCallStatus::Completed,
                };
                let receiver_id = ev.receiver_thread_id.to_string();
                let agents_states = [(receiver_id.clone(), CollabAgentState::from(ev.status))]
                    .into_iter()
                    .collect();
                let item = ThreadItem::CollabAgentToolCall {
                    id: ev.call_id,
                    tool: CollabAgentTool::CloseAgent,
                    status,
                    sender_thread_id: ev.sender_thread_id.to_string(),
                    receiver_thread_ids: vec![receiver_id],
                    prompt: None,
                    agents_states,
                };
                vec![ServerNotification::ItemCompleted(
                    ItemCompletedNotification {
                        thread_id: self.thread_id.to_string(),
                        turn_id,
                        item,
                    },
                )]
            }

            EventMsg::EnteredReviewMode(ev) => {
                let review = ev
                    .user_facing_hint
                    .unwrap_or_else(|| format!("Reviewing: {:?}", ev.target));
                let item = ThreadItem::EnteredReviewMode {
                    id: turn_id.clone(),
                    review,
                };
                vec![
                    ServerNotification::ItemStarted(ItemStartedNotification {
                        thread_id: self.thread_id.to_string(),
                        turn_id: turn_id.clone(),
                        item: item.clone(),
                    }),
                    ServerNotification::ItemCompleted(ItemCompletedNotification {
                        thread_id: self.thread_id.to_string(),
                        turn_id,
                        item,
                    }),
                ]
            }

            EventMsg::ExitedReviewMode(ev) => {
                let review = match ev.review_output {
                    Some(output) => format!("Review completed: {output:?}"),
                    None => "Review completed".to_string(),
                };
                let item = ThreadItem::ExitedReviewMode {
                    id: turn_id.clone(),
                    review,
                };
                vec![
                    ServerNotification::ItemStarted(ItemStartedNotification {
                        thread_id: self.thread_id.to_string(),
                        turn_id: turn_id.clone(),
                        item: item.clone(),
                    }),
                    ServerNotification::ItemCompleted(ItemCompletedNotification {
                        thread_id: self.thread_id.to_string(),
                        turn_id,
                        item,
                    }),
                ]
            }

            _ => {
                tracing::debug!(
                    "Unhandled event type: {} (Phase 1 core + file/command events)",
                    std::any::type_name_of_val(&msg)
                );
                vec![]
            }
        }
    }

    pub fn event_type_name(notification: &ServerNotification) -> &'static str {
        match notification {
            ServerNotification::Error(_) => "error",
            ServerNotification::ThreadStarted(_) => "thread/started",
            ServerNotification::ThreadStatusChanged(_) => "thread/status/changed",
            ServerNotification::ThreadArchived(_) => "thread/archived",
            ServerNotification::ThreadUnarchived(_) => "thread/unarchived",
            ServerNotification::ThreadTokenUsageUpdated(_) => "thread/tokenUsage/updated",
            ServerNotification::TurnStarted(_) => "turn/started",
            ServerNotification::TurnCompleted(_) => "turn/completed",
            ServerNotification::TurnDiffUpdated(_) => "turn/diff/updated",
            ServerNotification::TurnPlanUpdated(_) => "turn/plan/updated",
            ServerNotification::ItemStarted(_) => "item/started",
            ServerNotification::ItemCompleted(_) => "item/completed",
            ServerNotification::RawResponseItemCompleted(_) => "rawResponseItem/completed",
            ServerNotification::AgentMessageDelta(_) => "item/agentMessage/delta",
            ServerNotification::CommandExecutionOutputDelta(_) => {
                "item/commandExecution/outputDelta"
            }
            ServerNotification::TerminalInteraction(_) => {
                "item/commandExecution/terminalInteraction"
            }
            ServerNotification::FileChangeOutputDelta(_) => "item/fileChange/outputDelta",
            ServerNotification::McpToolCallProgress(_) => "item/mcpToolCall/progress",
            ServerNotification::McpServerOauthLoginCompleted(_) => "mcpServer/oauthLogin/completed",
            ServerNotification::AccountUpdated(_) => "account/updated",
            ServerNotification::AccountRateLimitsUpdated(_) => "account/rateLimits/updated",
            ServerNotification::AppListUpdated(_) => "app/list/updated",
            ServerNotification::ReasoningSummaryTextDelta(_) => "item/reasoning/summaryTextDelta",
            ServerNotification::ReasoningSummaryPartAdded(_) => "item/reasoning/summaryPartAdded",
            ServerNotification::ReasoningTextDelta(_) => "item/reasoning/textDelta",
            ServerNotification::ContextCompacted(_) => "thread/compacted",
            ServerNotification::ModelRerouted(_) => "model/rerouted",
            ServerNotification::DeprecationNotice(_) => "deprecationNotice",
            ServerNotification::ConfigWarning(_) => "configWarning",
            ServerNotification::FuzzyFileSearchSessionUpdated(_) => {
                "fuzzyFileSearch/sessionUpdated"
            }
            ServerNotification::FuzzyFileSearchSessionCompleted(_) => {
                "fuzzyFileSearch/sessionCompleted"
            }
            ServerNotification::WindowsWorldWritableWarning(_) => "windows/worldWritableWarning",
            ServerNotification::WindowsSandboxSetupCompleted(_) => "windowsSandbox/setupCompleted",
            ServerNotification::AccountLoginCompleted(_) => "account/login/completed",
            ServerNotification::AuthStatusChange(_) => "authStatusChange",
            ServerNotification::LoginChatGptComplete(_) => "loginChatGptComplete",
            ServerNotification::SessionConfigured(_) => "sessionConfigured",
            ServerNotification::ThreadNameUpdated(_) => "thread/name/updated",
            ServerNotification::PlanDelta(_) => "item/plan/delta",
        }
    }
}
