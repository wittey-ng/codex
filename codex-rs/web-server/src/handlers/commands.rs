use axum::Json;
use axum::extract::State;
use serde::Deserialize;
use serde::Serialize;
use std::path::PathBuf;
use std::process::Stdio;
use std::result::Result;
use std::time::Duration;
use tokio::process::Command;
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

    // Build command
    let (program, args) = req
        .command
        .split_first()
        .ok_or_else(|| ApiError::InvalidRequest("Command cannot be empty".to_string()))?;

    let mut cmd = Command::new(program);
    cmd.args(args)
        .current_dir(&cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null());

    // Execute with 10s timeout
    let output = tokio::time::timeout(Duration::from_secs(10), cmd.output())
        .await
        .map_err(|_| ApiError::Timeout("Command exceeded 10s timeout".to_string()))?
        .map_err(|e| ApiError::InternalError(format!("Command execution failed: {e}")))?;

    // Convert output to strings
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let exit_code = output.status.code().unwrap_or(-1);

    // Truncate output if too large (1MB limit)
    let max_output_size = 1_048_576; // 1MB
    let stdout = if stdout.len() > max_output_size {
        let truncated = &stdout[..max_output_size];
        format!("{truncated}... (truncated)")
    } else {
        stdout
    };

    let stderr = if stderr.len() > max_output_size {
        let truncated = &stderr[..max_output_size];
        format!("{truncated}... (truncated)")
    } else {
        stderr
    };

    Ok(Json(ExecuteCommandResponse {
        stdout,
        stderr,
        exit_code,
    }))
}
