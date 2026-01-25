use codex_protocol::ThreadId;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use tokio::sync::Mutex;

use crate::state::ApprovalContext;
use crate::state::ApprovalDecision;
use crate::state::ApprovalResponse;
use crate::state::ApprovalType;

pub struct ApprovalManager {
    pending_approvals: Arc<Mutex<HashMap<String, ApprovalContext>>>,
}

impl ApprovalManager {
    pub fn new(pending_approvals: Arc<Mutex<HashMap<String, ApprovalContext>>>) -> Self {
        Self { pending_approvals }
    }

    /// Register a new approval request
    #[allow(dead_code)]
    pub async fn register_approval(
        &self,
        approval_id: String,
        thread_id: ThreadId,
        item_id: String,
        approval_type: ApprovalType,
        response_channel: tokio::sync::oneshot::Sender<ApprovalResponse>,
        timeout: Duration,
    ) {
        let context = ApprovalContext {
            thread_id,
            item_id,
            approval_type,
            response_channel,
            created_at: Instant::now(),
            timeout,
        };

        let mut approvals = self.pending_approvals.lock().await;
        approvals.insert(approval_id, context);
    }

    /// Respond to an approval request
    pub async fn respond_to_approval(
        &self,
        approval_id: &str,
        decision: ApprovalDecision,
    ) -> Result<(), String> {
        let mut approvals = self.pending_approvals.lock().await;

        if let Some(context) = approvals.remove(approval_id) {
            // Check if approval has timed out
            if context.created_at.elapsed() >= context.timeout {
                return Err("Approval request has timed out".to_string());
            }

            let response = ApprovalResponse { decision };

            // Send response through channel
            context
                .response_channel
                .send(response)
                .map_err(|_| "Failed to send approval response".to_string())?;

            Ok(())
        } else {
            Err("Approval request not found".to_string())
        }
    }

    /// Clean up expired approval requests
    #[allow(dead_code)]
    pub async fn cleanup_expired(&self) {
        let mut approvals = self.pending_approvals.lock().await;
        approvals.retain(|_, ctx| ctx.created_at.elapsed() < ctx.timeout);
    }

    /// Get approval context (for inspection)
    #[allow(dead_code)]
    pub async fn get_approval(&self, approval_id: &str) -> Option<ApprovalInfo> {
        let approvals = self.pending_approvals.lock().await;
        approvals.get(approval_id).map(|ctx| ApprovalInfo {
            thread_id: ctx.thread_id.to_string(),
            item_id: ctx.item_id.clone(),
            approval_type: ctx.approval_type.clone(),
            elapsed: ctx.created_at.elapsed(),
            timeout: ctx.timeout,
        })
    }
}

/// Public approval information (without sensitive channel data)
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ApprovalInfo {
    pub thread_id: String,
    pub item_id: String,
    pub approval_type: ApprovalType,
    pub elapsed: Duration,
    pub timeout: Duration,
}
