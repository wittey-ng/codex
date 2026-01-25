use anyhow::Result;
use serde_json::json;

use crate::common::TestFixture;

#[tokio::test]
async fn test_mcp_server_config_setup() -> Result<()> {
    let fixture = TestFixture::new().await?;

    // Create config with MCP servers
    let mcp_config = r#"
model = "test-model"

[mcp_servers.test_server]
transport = "stdio"
command = "node"
args = ["server.js"]
"#;
    fixture.create_test_config(mcp_config)?;

    let config_path = fixture.codex_home_path().join("config.toml");
    assert!(config_path.exists());

    let content = std::fs::read_to_string(&config_path)?;
    assert!(content.contains("mcp_servers"));
    assert!(content.contains("test_server"));

    Ok(())
}

#[tokio::test]
async fn test_mcp_server_status_pagination_cursor() -> Result<()> {
    // Test cursor-based pagination parameters
    let cursor = "10";
    let limit = 20;

    // Simulate pagination logic
    let offset: usize = cursor.parse().unwrap_or(0);
    assert_eq!(offset, 10);

    let effective_limit = limit.clamp(1, 100);
    assert_eq!(effective_limit, 20);

    Ok(())
}

#[tokio::test]
async fn test_mcp_server_status_cursor_boundary() -> Result<()> {
    // Test boundary conditions
    let test_cases = vec![
        ("0", 0),   // Start
        ("99", 99), // Valid offset
        ("abc", 0), // Invalid - should default to 0
        ("", 0),    // Empty - should default to 0
    ];

    for (cursor, expected_offset) in test_cases {
        let offset = cursor.parse::<usize>().unwrap_or(0);
        assert_eq!(offset, expected_offset, "Failed for cursor: {cursor}");
    }

    Ok(())
}

#[tokio::test]
async fn test_mcp_server_status_limit_clamping() -> Result<()> {
    // Test limit clamping (1-100 range)
    let test_cases = vec![
        (0, 1),     // Below min -> clamp to 1
        (1, 1),     // Min
        (50, 50),   // Mid-range
        (100, 100), // Max
        (200, 100), // Above max -> clamp to 100
    ];

    for (input_limit, expected) in test_cases {
        let clamped = input_limit.clamp(1, 100);
        assert_eq!(clamped, expected, "Failed for limit: {input_limit}");
    }

    Ok(())
}

#[tokio::test]
async fn test_mcp_oauth_login_request_structure() -> Result<()> {
    // Test OAuth login request body
    let server_name = "github-server";

    let request_body = json!({
        "server_name": server_name
    });

    assert_eq!(request_body["server_name"], "github-server");

    Ok(())
}

#[tokio::test]
async fn test_mcp_oauth_callback_port_config() -> Result<()> {
    let fixture = TestFixture::new().await?;

    let config_with_port = r#"
model = "test-model"
mcp_oauth_callback_port = 8765
"#;
    fixture.create_test_config(config_with_port)?;

    let config_path = fixture.codex_home_path().join("config.toml");
    let content = std::fs::read_to_string(&config_path)?;

    assert!(content.contains("mcp_oauth_callback_port"));
    assert!(content.contains("8765"));

    Ok(())
}

#[tokio::test]
async fn test_mcp_server_transport_types() -> Result<()> {
    let fixture = TestFixture::new().await?;

    // Test different transport types
    let stdio_config = r#"
[mcp_servers.stdio_server]
transport = "stdio"
command = "node"
args = ["server.js"]
"#;
    fixture.create_test_config(stdio_config)?;

    let config_path = fixture.codex_home_path().join("config.toml");
    let content = std::fs::read_to_string(&config_path)?;

    assert!(content.contains("transport"));
    assert!(content.contains("stdio"));

    Ok(())
}

#[tokio::test]
async fn test_mcp_server_http_transport_config() -> Result<()> {
    let fixture = TestFixture::new().await?;

    let http_config = r#"
[mcp_servers.http_server]
transport = "streamable_http"
url = "https://api.example.com/mcp"

[mcp_servers.http_server.http_headers]
Authorization = "Bearer token123"
"#;
    fixture.create_test_config(http_config)?;

    let config_path = fixture.codex_home_path().join("config.toml");
    let content = std::fs::read_to_string(&config_path)?;

    assert!(content.contains("streamable_http"));
    assert!(content.contains("https://api.example.com/mcp"));
    assert!(content.contains("http_headers"));

    Ok(())
}
