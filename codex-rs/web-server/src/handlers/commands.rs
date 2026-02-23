use axum::Json;
use axum::extract::State;
use codex_core::config::Config;
use codex_core::error::CodexErr;
use codex_core::error::SandboxErr;
use codex_core::exec::ExecExpiration;
use codex_core::exec::ExecParams;
use codex_core::exec::SandboxType;
use codex_core::exec::process_exec_tool_call;
use codex_core::exec_env::create_env;
use codex_core::features::Feature;
use codex_core::get_platform_sandbox;
use codex_core::sandboxing::SandboxPermissions;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::protocol::SandboxPolicy;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::result::Result;
use utoipa::ToSchema;

use crate::error::ApiError;
use crate::state::WebServerState;

#[derive(Debug, Deserialize, ToSchema)]
pub struct ExecuteCommandRequest {
    pub command: Vec<String>,
    pub cwd: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ExecuteCommandResponse {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

/// POST /api/v2/commands
///
/// Executes a one-off command outside of thread context (with 10s timeout)
#[utoipa::path(
    post,
    path = "/api/v2/commands",
    request_body = ExecuteCommandRequest,
    responses(
        (status = 200, description = "Command executed successfully", body = ExecuteCommandResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 504, description = "Command timeout (exceeded 10s)"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Commands"
)]
pub async fn execute_command(
    State(state): State<WebServerState>,
    Json(req): Json<ExecuteCommandRequest>,
) -> Result<Json<ExecuteCommandResponse>, ApiError> {
    // Validate command
    if req.command.is_empty() {
        return Err(ApiError::InvalidRequest(
            "Command cannot be empty".to_string(),
        ));
    }

    // Validate and canonicalize CWD (prevent path traversal)
    let cwd = if let Some(cwd_str) = req.cwd {
        let cwd_path = PathBuf::from(&cwd_str);

        // Ensure the path is within codex_home or a safe directory
        let canonical_cwd = cwd_path
            .canonicalize()
            .map_err(|e| ApiError::InvalidRequest(format!("Invalid cwd: {e}")))?;

        // Basic path traversal check (ensure it's an absolute path)
        if !canonical_cwd.is_absolute() {
            return Err(ApiError::InvalidRequest(
                "CWD must be an absolute path".to_string(),
            ));
        }

        canonical_cwd
    } else {
        // Use codex_home as default
        state.codex_home.clone()
    };

    let config = Config::load_with_cli_overrides(vec![])
        .await
        .map_err(|e| ApiError::InternalError(format!("Failed to load config: {e}")))?;

    let sandbox_policy = config.permissions.sandbox_policy.get();
    if matches!(
        sandbox_policy,
        SandboxPolicy::DangerFullAccess | SandboxPolicy::ExternalSandbox { .. }
    ) {
        return Err(ApiError::InternalError(
            "Refusing to execute commands with sandbox_policy=DangerFullAccess/ExternalSandbox"
                .to_string(),
        ));
    }
    if get_platform_sandbox(false) != Some(SandboxType::BoxLite) {
        return Err(ApiError::InternalError(
            "BoxLite sandbox is required for /api/v2/commands; configure BOXLITE_RUNTIME_DIR so BoxLite can locate boxlite-guest/mke2fs/debugfs"
                .to_string(),
        ));
    }

    let env: HashMap<String, String> =
        create_env(&config.permissions.shell_environment_policy, None);

    let params = ExecParams {
        command: req.command,
        cwd: cwd.clone(),
        expiration: ExecExpiration::Timeout(std::time::Duration::from_secs(10)),
        env,
        network: None,
        sandbox_permissions: SandboxPermissions::UseDefault,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
        justification: None,
        arg0: None,
    };

    let use_linux_sandbox_bwrap = config.features.enabled(Feature::UseLinuxSandboxBwrap);
    let output = process_exec_tool_call(
        params,
        sandbox_policy,
        &cwd,
        &config.codex_linux_sandbox_exe,
        use_linux_sandbox_bwrap,
        None,
    )
    .await
    .map_err(|err| match err {
        CodexErr::Sandbox(SandboxErr::Timeout { .. }) => {
            ApiError::Timeout("Command exceeded 10s timeout".to_string())
        }
        CodexErr::InvalidRequest(message) | CodexErr::UnsupportedOperation(message) => {
            ApiError::InvalidRequest(message)
        }
        other => ApiError::InternalError(other.to_string()),
    })?;

    let stdout = output.stdout.text;
    let stderr = output.stderr.text;
    let exit_code = output.exit_code;

    Ok(Json(ExecuteCommandResponse {
        stdout,
        stderr,
        exit_code,
    }))
}
