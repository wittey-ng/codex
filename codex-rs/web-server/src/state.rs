use codex_core::ThreadManager;
use codex_core::auth::AuthManager;
use codex_core::config::service::ConfigService;
use codex_feedback::CodexFeedback;
use codex_protocol::ThreadId;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use tokio::sync::Mutex;
use tokio::sync::RwLock;
use utoipa::ToSchema;

#[derive(Clone)]
pub struct WebServerState {
    pub thread_manager: Arc<ThreadManager>,
    pub auth_manager: Arc<AuthManager>,
    pub config_service: Arc<ConfigService>,
    pub codex_home: PathBuf,
    pub attachments_dir: PathBuf,
    pub auth_token: String,
    pub sessions: Arc<RwLock<SessionStore>>,
    pub pending_approvals: Arc<Mutex<HashMap<String, ApprovalContext>>>,
    pub feedback: CodexFeedback,
}

impl WebServerState {
    pub fn new(
        thread_manager: Arc<ThreadManager>,
        auth_manager: Arc<AuthManager>,
        config_service: Arc<ConfigService>,
        codex_home: PathBuf,
        attachments_dir: PathBuf,
        auth_token: String,
        feedback: CodexFeedback,
    ) -> Self {
        Self {
            thread_manager,
            auth_manager,
            config_service,
            codex_home,
            attachments_dir,
            auth_token,
            sessions: Arc::new(RwLock::new(SessionStore::new())),
            pending_approvals: Arc::new(Mutex::new(HashMap::new())),
            feedback,
        }
    }
}

pub struct SessionStore {
    active_streams: HashMap<ThreadId, usize>,
}

impl Default for SessionStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionStore {
    pub fn new() -> Self {
        Self {
            active_streams: HashMap::new(),
        }
    }

    pub fn register_stream(&mut self, thread_id: ThreadId) {
        *self.active_streams.entry(thread_id).or_insert(0) += 1;
    }

    pub fn unregister_stream(&mut self, thread_id: ThreadId) {
        if let Some(count) = self.active_streams.get_mut(&thread_id) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                self.active_streams.remove(&thread_id);
            }
        }
    }
}

#[allow(dead_code)]
pub struct ApprovalContext {
    pub thread_id: ThreadId,
    pub item_id: String,
    pub approval_type: ApprovalType,
    pub response_channel: tokio::sync::oneshot::Sender<ApprovalResponse>,
    pub created_at: Instant,
    pub timeout: Duration,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum ApprovalType {
    CommandExecution {
        command: Vec<String>,
        cwd: PathBuf,
        reason: String,
    },
    FileChange {
        reason: String,
    },
}

#[derive(Debug, Clone)]
pub struct ApprovalResponse {
    pub decision: ApprovalDecision,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ApprovalDecision {
    Approve,
    Decline,
}
