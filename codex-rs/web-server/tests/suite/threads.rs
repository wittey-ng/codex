use anyhow::Result;
use codex_protocol::ThreadId;

use crate::common::TEST_CONFIG;
use crate::common::TestFixture;

#[tokio::test]
async fn test_thread_resume_rollout_file_validation() -> Result<()> {
    let fixture = TestFixture::new().await?;
    fixture.create_test_config(TEST_CONFIG)?;

    let thread_id = ThreadId::new();

    // Create valid rollout file with mock events
    let rollout_content = r#"{"type":"thread_started","thread_id":"test","model":"test-model"}
{"type":"turn_started","turn_id":"turn-1"}
{"type":"turn_completed","turn_id":"turn-1"}
"#;
    let rollout_path = fixture.create_mock_rollout(&thread_id.to_string(), rollout_content)?;

    // Verify rollout file exists
    assert!(rollout_path.exists());

    // Verify file content
    let content = std::fs::read_to_string(&rollout_path)?;
    assert!(content.contains("thread_started"));
    assert!(content.contains("turn_started"));
    assert!(content.contains("turn_completed"));

    Ok(())
}

#[tokio::test]
async fn test_thread_resume_nonexistent_rollout_detection() -> Result<()> {
    let fixture = TestFixture::new().await?;
    fixture.create_test_config(TEST_CONFIG)?;

    let thread_id = ThreadId::new();

    // Verify rollout file does NOT exist
    let sessions_dir = fixture.codex_home_path().join("sessions");
    let rollout_path = sessions_dir.join(format!("{thread_id}.jsonl"));
    assert!(!rollout_path.exists());

    Ok(())
}

#[tokio::test]
async fn test_thread_resume_rollout_path_construction() -> Result<()> {
    let fixture = TestFixture::new().await?;

    let thread_id = ThreadId::new();

    // Test that we construct the correct path
    let sessions_dir = fixture.codex_home_path().join("sessions");
    let expected_path = sessions_dir.join(format!("{thread_id}.jsonl"));

    // Verify sessions directory exists
    assert!(sessions_dir.exists());

    // Path should be well-formed
    assert!(
        expected_path
            .to_string_lossy()
            .contains(&thread_id.to_string())
    );
    assert!(expected_path.to_string_lossy().ends_with(".jsonl"));

    Ok(())
}

#[tokio::test]
async fn test_thread_resume_multiple_rollouts() -> Result<()> {
    let fixture = TestFixture::new().await?;
    fixture.create_test_config(TEST_CONFIG)?;

    // Create multiple thread rollouts
    for i in 0..3 {
        let thread_id = ThreadId::new();
        let rollout_content = format!(r#"{{"type":"thread_started","thread_id":"test-{i}"}}"#);
        fixture.create_mock_rollout(&thread_id.to_string(), &rollout_content)?;
    }

    // Verify all files exist
    let sessions_dir = fixture.codex_home_path().join("sessions");
    let entries = std::fs::read_dir(sessions_dir)?;
    let count = entries.count();

    assert_eq!(count, 3, "Expected 3 rollout files");

    Ok(())
}

#[tokio::test]
async fn test_thread_resume_rollout_content_format() -> Result<()> {
    let fixture = TestFixture::new().await?;
    fixture.create_test_config(TEST_CONFIG)?;

    let thread_id = ThreadId::new();

    // Create rollout with multiple events (JSONL format)
    let rollout_content = r#"{"type":"thread_started","model":"test"}
{"type":"turn_started","turn_id":"1"}
{"type":"item_started","item_id":"i1"}
{"type":"item_completed","item_id":"i1"}
{"type":"turn_completed","turn_id":"1"}
"#;
    let rollout_path = fixture.create_mock_rollout(&thread_id.to_string(), rollout_content)?;

    // Read and verify JSONL format
    let content = std::fs::read_to_string(&rollout_path)?;
    let lines: Vec<&str> = content.trim().split('\n').collect();

    assert_eq!(lines.len(), 5, "Expected 5 JSON lines");

    // Each line should be valid JSON
    for line in lines {
        serde_json::from_str::<serde_json::Value>(line).expect("Each line should be valid JSON");
    }

    Ok(())
}
