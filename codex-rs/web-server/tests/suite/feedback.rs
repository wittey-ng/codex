use anyhow::Result;
use codex_protocol::ThreadId;
use serde_json::json;

use crate::common::TEST_CONFIG;
use crate::common::TestFixture;

#[tokio::test]
async fn test_feedback_upload_fixture_setup() -> Result<()> {
    let fixture = TestFixture::new().await?;
    fixture.create_test_config(TEST_CONFIG)?;

    // Verify fixture was created successfully
    assert!(fixture.codex_home_path().exists());
    assert!(fixture.attachments_path().exists());

    Ok(())
}

#[tokio::test]
async fn test_feedback_rollout_file_creation() -> Result<()> {
    let fixture = TestFixture::new().await?;
    fixture.create_test_config(TEST_CONFIG)?;

    let thread_id = ThreadId::new();

    // Create mock rollout file
    let rollout_content = r#"{"type":"thread_started","thread_id":"test"}"#;
    fixture.create_mock_rollout(&thread_id.to_string(), rollout_content)?;

    // Verify rollout file was created
    let sessions_dir = fixture.codex_home_path().join("sessions");
    let rollout_path = sessions_dir.join(format!("{thread_id}.jsonl"));
    assert!(rollout_path.exists());

    // Verify content
    let content = std::fs::read_to_string(&rollout_path)?;
    assert!(content.contains("thread_started"));

    Ok(())
}

#[tokio::test]
async fn test_feedback_request_body_structure() -> Result<()> {
    // Test that we can construct valid request bodies
    let request_body = json!({
        "classification": "bug",
        "reason": "Something went wrong",
        "include_logs": false
    });

    assert_eq!(request_body["classification"], "bug");
    assert_eq!(request_body["include_logs"], false);

    Ok(())
}

#[tokio::test]
async fn test_feedback_with_thread_id_structure() -> Result<()> {
    let thread_id = ThreadId::new();

    let request_body = json!({
        "classification": "good_result",
        "reason": "Great response!",
        "thread_id": thread_id.to_string(),
        "include_logs": true
    });

    assert_eq!(request_body["classification"], "good_result");
    assert_eq!(request_body["thread_id"], thread_id.to_string());

    Ok(())
}
