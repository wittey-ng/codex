use anyhow::Result;
use std::path::PathBuf;
use tempfile::TempDir;

/// Test fixture containing temporary directories
pub struct TestFixture {
    pub codex_home: TempDir,
    pub attachments_dir: TempDir,
}

impl TestFixture {
    /// Create a new test fixture with temp directories
    pub async fn new() -> Result<Self> {
        let codex_home = TempDir::new()?;
        let attachments_dir = TempDir::new()?;

        // Create sessions directory
        let sessions_dir = codex_home.path().join("sessions");
        std::fs::create_dir_all(&sessions_dir)?;

        Ok(Self {
            codex_home,
            attachments_dir,
        })
    }

    /// Get the codex home path
    pub fn codex_home_path(&self) -> PathBuf {
        self.codex_home.path().to_path_buf()
    }

    /// Get the attachments directory path
    pub fn attachments_path(&self) -> PathBuf {
        self.attachments_dir.path().to_path_buf()
    }

    /// Create a test config.toml in the codex home
    pub fn create_test_config(&self, content: &str) -> Result<()> {
        let config_path = self.codex_home.path().join("config.toml");
        std::fs::write(config_path, content)?;
        Ok(())
    }

    /// Create a mock rollout file for a thread
    pub fn create_mock_rollout(&self, thread_id: &str, content: &str) -> Result<PathBuf> {
        let sessions_dir = self.codex_home.path().join("sessions");
        let rollout_path = sessions_dir.join(format!("{thread_id}.jsonl"));
        std::fs::write(&rollout_path, content)?;
        Ok(rollout_path)
    }
}

/// Default test config content
pub const TEST_CONFIG: &str = r#"
model = "test-model"
approval_policy = "never"
sandbox_mode = "read-only"
"#;
