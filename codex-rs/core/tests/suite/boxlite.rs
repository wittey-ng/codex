#![cfg(not(target_os = "windows"))]

use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use codex_core::error::CodexErr;
use codex_core::error::Result;
use codex_core::error::SandboxErr;
use codex_core::exec::ExecExpiration;
use codex_core::exec::ExecParams;
use codex_core::exec::ExecToolCallOutput;
use codex_core::exec::SandboxType;
use codex_core::exec::process_exec_tool_call;
use codex_core::get_platform_sandbox;
use codex_core::sandboxing::SandboxPermissions;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::protocol::SandboxPolicy;
use pretty_assertions::assert_eq;
use serial_test::serial;

#[derive(Debug, PartialEq)]
struct ExecSnapshot {
    exit_code: i32,
    stdout: String,
    stderr: String,
    aggregated: String,
    timed_out: bool,
}

impl From<&ExecToolCallOutput> for ExecSnapshot {
    fn from(output: &ExecToolCallOutput) -> Self {
        Self {
            exit_code: output.exit_code,
            stdout: output.stdout.text.clone(),
            stderr: output.stderr.text.clone(),
            aggregated: output.aggregated_output.text.clone(),
            timed_out: output.timed_out,
        }
    }
}

fn boxlite_tests_enabled() -> bool {
    match std::env::var("CODEX_BOXLITE_TESTS") {
        Ok(value) => value == "1" || value.eq_ignore_ascii_case("true"),
        Err(_) => false,
    }
}

fn ensure_boxlite_enabled() -> bool {
    if !boxlite_tests_enabled() {
        eprintln!("Skipping BoxLite integration tests; set CODEX_BOXLITE_TESTS=1 to enable.");
        return false;
    }

    match get_platform_sandbox(false) {
        Some(SandboxType::BoxLite) => true,
        other => {
            eprintln!("Skipping BoxLite integration tests; sandbox is {other:?}.");
            false
        }
    }
}

fn default_expiration() -> ExecExpiration {
    ExecExpiration::Timeout(Duration::from_secs(30))
}

async fn run_boxlite(
    command: &[&str],
    cwd: &str,
    expiration: ExecExpiration,
) -> Result<ExecToolCallOutput> {
    let params = ExecParams {
        command: command.iter().map(ToString::to_string).collect(),
        cwd: PathBuf::from(cwd),
        expiration,
        env: HashMap::new(),
        network: None,
        sandbox_permissions: SandboxPermissions::UseDefault,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
        justification: None,
        arg0: None,
    };

    let policy = SandboxPolicy::new_read_only_policy();
    process_exec_tool_call(params, &policy, Path::new(cwd), &None, false, None).await
}

#[tokio::test]
#[serial]
async fn boxlite_echo_outputs_stdout() {
    if !ensure_boxlite_enabled() {
        return;
    }

    let output = run_boxlite(&["sh", "-c", "echo hello"], "/", default_expiration())
        .await
        .expect("expected exec to succeed");
    let snapshot = ExecSnapshot::from(&output);
    assert_eq!(
        snapshot,
        ExecSnapshot {
            exit_code: 0,
            stdout: "hello\n".to_string(),
            stderr: String::new(),
            aggregated: "hello\n".to_string(),
            timed_out: false,
        }
    );
}

#[tokio::test]
#[serial]
async fn boxlite_cwd_is_applied() {
    if !ensure_boxlite_enabled() {
        return;
    }

    let output = run_boxlite(&["sh", "-c", "pwd"], "/tmp", default_expiration())
        .await
        .expect("expected exec to succeed");
    let snapshot = ExecSnapshot::from(&output);
    assert_eq!(
        snapshot,
        ExecSnapshot {
            exit_code: 0,
            stdout: "/tmp\n".to_string(),
            stderr: String::new(),
            aggregated: "/tmp\n".to_string(),
            timed_out: false,
        }
    );
}

#[tokio::test]
#[serial]
async fn boxlite_stderr_is_captured() {
    if !ensure_boxlite_enabled() {
        return;
    }

    let output = run_boxlite(&["sh", "-c", "echo err 1>&2"], "/", default_expiration())
        .await
        .expect("expected exec to succeed");
    let snapshot = ExecSnapshot::from(&output);
    assert_eq!(
        snapshot,
        ExecSnapshot {
            exit_code: 0,
            stdout: String::new(),
            stderr: "err\n".to_string(),
            aggregated: "err\n".to_string(),
            timed_out: false,
        }
    );
}

#[tokio::test]
#[serial]
async fn boxlite_non_zero_exit_code_is_returned() {
    if !ensure_boxlite_enabled() {
        return;
    }

    let output = run_boxlite(&["sh", "-c", "exit 2"], "/", default_expiration())
        .await
        .expect("expected exec to succeed");
    let snapshot = ExecSnapshot::from(&output);
    assert_eq!(
        snapshot,
        ExecSnapshot {
            exit_code: 2,
            stdout: String::new(),
            stderr: String::new(),
            aggregated: String::new(),
            timed_out: false,
        }
    );
}

#[tokio::test]
#[serial]
async fn boxlite_timeout_returns_sandbox_timeout() {
    if !ensure_boxlite_enabled() {
        return;
    }

    let err = run_boxlite(
        &["sh", "-c", "sleep 60"],
        "/",
        ExecExpiration::Timeout(Duration::from_millis(50)),
    )
    .await
    .expect_err("expected timeout");
    let output = match err {
        CodexErr::Sandbox(SandboxErr::Timeout { output }) => output,
        other => panic!("expected timeout, got {other:?}"),
    };
    let snapshot = ExecSnapshot::from(output.as_ref());
    assert_eq!(
        snapshot,
        ExecSnapshot {
            exit_code: 124,
            stdout: String::new(),
            stderr: "Execution timed out or was cancelled".to_string(),
            aggregated: "Execution timed out or was cancelled".to_string(),
            timed_out: true,
        }
    );
}
