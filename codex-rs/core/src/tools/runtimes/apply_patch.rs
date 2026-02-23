//! Apply Patch runtime: executes verified patches under the orchestrator.
//!
//! Assumes `apply_patch` verification/approval happened upstream. Reuses that
//! decision to avoid re-prompting, builds the self-invocation command for
//! `codex --codex-run-as-apply-patch`, and runs under the current
//! `SandboxAttempt` with a minimal environment.
use crate::config::types::ShellEnvironmentPolicy;
use crate::config::types::ShellEnvironmentPolicyInherit;
use crate::exec::ExecToolCallOutput;
use crate::exec_env;
use crate::sandboxing::CommandSpec;
use crate::sandboxing::SandboxPermissions;
use crate::sandboxing::execute_env;
use crate::tools::sandboxing::Approvable;
use crate::tools::sandboxing::ApprovalCtx;
use crate::tools::sandboxing::ExecApprovalRequirement;
use crate::tools::sandboxing::SandboxAttempt;
use crate::tools::sandboxing::Sandboxable;
use crate::tools::sandboxing::SandboxablePreference;
use crate::tools::sandboxing::ToolCtx;
use crate::tools::sandboxing::ToolError;
use crate::tools::sandboxing::ToolRuntime;
use crate::tools::sandboxing::with_cached_approval;
use codex_apply_patch::ApplyPatchAction;
use codex_apply_patch::CODEX_CORE_APPLY_PATCH_ARG1;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::FileChange;
use codex_protocol::protocol::ReviewDecision;
use codex_utils_absolute_path::AbsolutePathBuf;
use futures::future::BoxFuture;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::Path;
use std::path::PathBuf;

const LOADER_ENV_VARS: [&str; 4] = [
    "DYLD_LIBRARY_PATH",
    "DYLD_FALLBACK_LIBRARY_PATH",
    "DYLD_INSERT_LIBRARIES",
    "LD_LIBRARY_PATH",
];
const LOADER_PATH_ENV_VARS: [&str; 3] = [
    "DYLD_LIBRARY_PATH",
    "DYLD_FALLBACK_LIBRARY_PATH",
    "LD_LIBRARY_PATH",
];
const BOXLITE_RUNTIME_ENV_VAR: &str = "BOXLITE_RUNTIME_DIR";

#[derive(Debug)]
pub struct ApplyPatchRequest {
    pub action: ApplyPatchAction,
    pub file_paths: Vec<AbsolutePathBuf>,
    pub changes: std::collections::HashMap<PathBuf, FileChange>,
    pub exec_approval_requirement: ExecApprovalRequirement,
    pub timeout_ms: Option<u64>,
    pub codex_exe: Option<PathBuf>,
}

#[derive(Default)]
pub struct ApplyPatchRuntime;

impl ApplyPatchRuntime {
    pub fn new() -> Self {
        Self
    }

    fn minimal_env() -> HashMap<String, String> {
        let policy = ShellEnvironmentPolicy {
            inherit: ShellEnvironmentPolicyInherit::Core,
            ..ShellEnvironmentPolicy::default()
        };
        let mut env = exec_env::create_env(&policy, None);
        for key in LOADER_ENV_VARS {
            if let Ok(value) = std::env::var(key) {
                env.insert(key.to_string(), value);
            }
        }
        Self::apply_boxlite_runtime_env(&mut env);
        env
    }

    fn apply_boxlite_runtime_env(env: &mut HashMap<String, String>) {
        let runtime_dir = match Self::boxlite_runtime_dir() {
            Some(runtime_dir) => runtime_dir,
            None => return,
        };
        let runtime_str = runtime_dir.to_string_lossy().to_string();
        env.insert(BOXLITE_RUNTIME_ENV_VAR.to_string(), runtime_str);
        for key in LOADER_PATH_ENV_VARS {
            Self::prepend_env_path(env, key, &runtime_dir);
        }
    }

    fn boxlite_runtime_dir() -> Option<PathBuf> {
        if let Some(runtime_dir) = std::env::var_os(BOXLITE_RUNTIME_ENV_VAR) {
            return Some(PathBuf::from(runtime_dir));
        }
        let exe = std::env::current_exe().ok()?;
        let profile_dir = Self::profile_dir_from_exe(&exe)?;
        let deps_runtime_dir = profile_dir.join("deps").join("runtime");
        if deps_runtime_dir.join("mke2fs").is_file() {
            return Some(deps_runtime_dir);
        }
        let build_runtime_dir = Self::discover_boxlite_runtime_dir(&profile_dir);
        if let Some(runtime_dir) = build_runtime_dir {
            return Some(runtime_dir);
        }
        None
    }

    fn profile_dir_from_exe(exe: &Path) -> Option<PathBuf> {
        let parent = exe.parent()?;
        let profile_dir = if parent.file_name() == Some(OsStr::new("deps")) {
            parent.parent()?
        } else {
            parent
        };
        Some(profile_dir.to_path_buf())
    }

    fn discover_boxlite_runtime_dir(profile_dir: &Path) -> Option<PathBuf> {
        let build_dir = profile_dir.join("build");
        let entries = std::fs::read_dir(&build_dir).ok()?;
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let name = file_name.to_string_lossy();
            if !name.starts_with("boxlite-") {
                continue;
            }
            let runtime_dir = entry.path().join("out").join("runtime");
            if runtime_dir.join("mke2fs").is_file() {
                return Some(runtime_dir);
            }
        }
        None
    }

    fn prepend_env_path(env: &mut HashMap<String, String>, key: &str, value: &Path) {
        let mut paths = vec![value.to_path_buf()];
        if let Some(existing) = env.get(key) {
            let existing_paths = std::env::split_paths(existing);
            for path in existing_paths {
                if path != *value {
                    paths.push(path);
                }
            }
        }
        if let Ok(joined) = std::env::join_paths(paths) {
            env.insert(key.to_string(), joined.to_string_lossy().to_string());
        }
    }

    fn build_command_spec(req: &ApplyPatchRequest) -> Result<CommandSpec, ToolError> {
        use std::env;
        let exe = if cfg!(target_os = "linux") {
            if let Some(path) = req.codex_exe.as_ref().filter(|path| path.exists()) {
                path.clone()
            } else {
                env::current_exe().map_err(|e| {
                    ToolError::Rejected(format!("failed to determine codex exe: {e}"))
                })?
            }
        } else {
            env::current_exe()
                .map_err(|e| ToolError::Rejected(format!("failed to determine codex exe: {e}")))?
        };
        let program = exe.to_string_lossy().to_string();
        Ok(CommandSpec {
            program,
            args: vec![
                CODEX_CORE_APPLY_PATCH_ARG1.to_string(),
                req.action.patch.clone(),
            ],
            cwd: req.action.cwd.clone(),
            expiration: req.timeout_ms.into(),
            // Keep env minimal but preserve loader vars needed to run the current binary.
            env: Self::minimal_env(),
            sandbox_permissions: SandboxPermissions::UseDefault,
            justification: None,
        })
    }

    fn stdout_stream(ctx: &ToolCtx<'_>) -> Option<crate::exec::StdoutStream> {
        Some(crate::exec::StdoutStream {
            sub_id: ctx.turn.sub_id.clone(),
            call_id: ctx.call_id.clone(),
            tx_event: ctx.session.get_tx_event(),
        })
    }
}

impl Sandboxable for ApplyPatchRuntime {
    fn sandbox_preference(&self) -> SandboxablePreference {
        SandboxablePreference::Auto
    }
    fn escalate_on_failure(&self) -> bool {
        true
    }
}

impl Approvable<ApplyPatchRequest> for ApplyPatchRuntime {
    type ApprovalKey = AbsolutePathBuf;

    fn approval_keys(&self, req: &ApplyPatchRequest) -> Vec<Self::ApprovalKey> {
        req.file_paths.clone()
    }

    fn start_approval_async<'a>(
        &'a mut self,
        req: &'a ApplyPatchRequest,
        ctx: ApprovalCtx<'a>,
    ) -> BoxFuture<'a, ReviewDecision> {
        let session = ctx.session;
        let turn = ctx.turn;
        let call_id = ctx.call_id.to_string();
        let retry_reason = ctx.retry_reason.clone();
        let approval_keys = self.approval_keys(req);
        let changes = req.changes.clone();
        Box::pin(async move {
            if let Some(reason) = retry_reason {
                let rx_approve = session
                    .request_patch_approval(turn, call_id, changes.clone(), Some(reason), None)
                    .await;
                return rx_approve.await.unwrap_or_default();
            }

            with_cached_approval(
                &session.services,
                "apply_patch",
                approval_keys,
                || async move {
                    let rx_approve = session
                        .request_patch_approval(turn, call_id, changes, None, None)
                        .await;
                    rx_approve.await.unwrap_or_default()
                },
            )
            .await
        })
    }

    fn wants_no_sandbox_approval(&self, policy: AskForApproval) -> bool {
        match policy {
            AskForApproval::Never => false,
            AskForApproval::Reject(reject_config) => !reject_config.rejects_sandbox_approval(),
            AskForApproval::OnFailure => true,
            AskForApproval::OnRequest => true,
            AskForApproval::UnlessTrusted => true,
        }
    }

    // apply_patch approvals are decided upstream by assess_patch_safety.
    //
    // This override ensures the orchestrator runs the patch approval flow when required instead
    // of falling back to the global exec approval policy.
    fn exec_approval_requirement(
        &self,
        req: &ApplyPatchRequest,
    ) -> Option<ExecApprovalRequirement> {
        Some(req.exec_approval_requirement.clone())
    }
}

impl ToolRuntime<ApplyPatchRequest, ExecToolCallOutput> for ApplyPatchRuntime {
    async fn run(
        &mut self,
        req: &ApplyPatchRequest,
        attempt: &SandboxAttempt<'_>,
        ctx: &ToolCtx<'_>,
    ) -> Result<ExecToolCallOutput, ToolError> {
        let spec = Self::build_command_spec(req)?;
        let env = attempt
            .env_for(spec, None)
            .map_err(|err| ToolError::Codex(err.into()))?;
        let out = execute_env(env, attempt.policy, Self::stdout_stream(ctx))
            .await
            .map_err(ToolError::Codex)?;
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::protocol::RejectConfig;

    #[test]
    fn wants_no_sandbox_approval_reject_respects_sandbox_flag() {
        let runtime = ApplyPatchRuntime::new();
        assert!(runtime.wants_no_sandbox_approval(AskForApproval::OnRequest));
        assert!(
            !runtime.wants_no_sandbox_approval(AskForApproval::Reject(RejectConfig {
                sandbox_approval: true,
                rules: false,
                mcp_elicitations: false,
            }))
        );
        assert!(
            runtime.wants_no_sandbox_approval(AskForApproval::Reject(RejectConfig {
                sandbox_approval: false,
                rules: false,
                mcp_elicitations: false,
            }))
        );
    }
}
