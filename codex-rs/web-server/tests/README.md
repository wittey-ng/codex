# Web Server Integration Tests

This directory contains integration tests for the Codex Web Server REST API.

## Test Structure

```
tests/
├── all.rs              # Main test entry point
├── common/
│   └── mod.rs          # Shared test utilities and fixtures
└── suite/
    ├── mod.rs          # Test suite aggregator
    ├── feedback.rs     # Feedback upload tests
    ├── threads.rs      # Thread resume tests
    ├── mcp.rs          # MCP server status and OAuth tests
    └── sse.rs          # SSE approval flow tests
```

## Running Tests

### Run all tests
```bash
cargo test -p codex-web-server
```

### Run specific test file
```bash
cargo test -p codex-web-server --test all -- feedback
cargo test -p codex-web-server --test all -- threads
cargo test -p codex-web-server --test all -- mcp
cargo test -p codex-web-server --test all -- sse
```

### Run a specific test
```bash
cargo test -p codex-web-server test_feedback_upload_fixture_setup
```

### Run with output
```bash
cargo test -p codex-web-server -- --nocapture
```

## Test Coverage

### Feedback Upload (`feedback.rs`) - 4 tests
- ✅ Fixture setup validation
- ✅ Rollout file creation and validation
- ✅ Request body structure validation
- ✅ Thread ID parameter validation

**Coverage**:
- File system operations (rollout files)
- Request body serialization
- Thread ID generation and validation

### Thread Resume (`threads.rs`) - 5 tests
- ✅ Rollout file validation
- ✅ Nonexistent rollout detection
- ✅ Rollout path construction
- ✅ Multiple rollouts handling
- ✅ JSONL format validation

**Coverage**:
- Rollout file creation and reading
- JSONL format parsing
- Path construction (`~/.codex/sessions/{thread_id}.jsonl`)
- Multi-threaded scenarios

### MCP Server (`mcp.rs`) - 8 tests
- ✅ MCP server config setup
- ✅ Pagination cursor parsing
- ✅ Cursor boundary conditions
- ✅ Limit clamping (1-100 range)
- ✅ OAuth login request structure
- ✅ OAuth callback port configuration
- ✅ STDIO transport configuration
- ✅ HTTP transport configuration

**Coverage**:
- Config file parsing
- Pagination logic (cursor + limit)
- Transport types (stdio, streamable_http)
- OAuth parameters

### SSE Approval Flow (`sse.rs`) - 9 tests
- ✅ SSE event type naming conventions
- ✅ Command execution approval request structure
- ✅ File change approval request structure
- ✅ Approval response structure (approve/decline)
- ✅ SSE keepalive format
- ✅ Approval context timeout (15 minutes)
- ✅ Approval ID uniqueness
- ✅ JSON encoding for SSE data
- ✅ Multiple approval requests isolation

**Coverage**:
- SSE event type format (`thread/started`, `item/commandExecution/requestApproval`)
- Approval request/response payloads
- Timeout handling
- Concurrent approval isolation

## Test Fixtures

### `TestFixture`
Located in `tests/common/mod.rs`, provides:
- Temporary `codex_home` directory
- Temporary `attachments` directory
- Helper methods for creating test configs and rollout files

**Example usage**:
```rust
use crate::common::{TestFixture, TEST_CONFIG};

#[tokio::test]
async fn test_example() -> Result<()> {
    let fixture = TestFixture::new().await?;
    fixture.create_test_config(TEST_CONFIG)?;

    // Test code here

    Ok(())
}
```

## Test Categories

### Unit Tests (Current Implementation)
Focus on individual components:
- File system operations
- Request/response structures
- Configuration parsing
- Path construction
- Pagination logic

### Integration Tests (Future)
Would require full HTTP server:
- End-to-end HTTP requests
- SSE event streaming
- Authentication middleware
- Approval flow (SSE → REST → Op submission)

## Adding New Tests

1. Create test file in `tests/suite/`
2. Add module declaration in `tests/suite/mod.rs`
3. Use `TestFixture` for setup
4. Follow naming convention: `test_{feature}_{scenario}`
5. Mark as `#[tokio::test]` for async tests

**Example**:
```rust
use anyhow::Result;
use crate::common::{TestFixture, TEST_CONFIG};

#[tokio::test]
async fn test_new_feature_scenario() -> Result<()> {
    let fixture = TestFixture::new().await?;
    fixture.create_test_config(TEST_CONFIG)?;

    // Your test logic
    assert!(some_condition);

    Ok(())
}
```

## CI/CD Integration

These tests are designed to run in CI/CD pipelines:
- No external dependencies (all mocked)
- Fast execution (< 1 second)
- Isolated (temp directories, no shared state)
- Deterministic (no timing dependencies)

## Known Limitations

1. **No HTTP Testing**: Tests validate logic but don't make actual HTTP requests
   - Requires `axum::test` or similar framework
   - Future enhancement: Add HTTP integration tests

2. **No Database/External Services**: All services are mocked
   - No real ThreadManager/AuthManager initialization
   - Future enhancement: Add test doubles

3. **No SSE Streaming**: SSE event handling not tested end-to-end
   - Future enhancement: Use SSE test client

## Performance

Current test suite:
- **26 tests**
- **Execution time**: ~10ms
- **Parallelization**: Enabled (tokio multi-thread)

## Debugging Tests

### Run with tracing
```bash
RUST_LOG=debug cargo test -p codex-web-server
```

### Run single test with output
```bash
cargo test -p codex-web-server test_name -- --nocapture --test-threads=1
```

### Check test binary
```bash
cargo test -p codex-web-server --no-run
```
