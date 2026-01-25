# Codex Web Server

REST API server for Codex CLI, providing HTTP/SSE interface as an alternative to the JSON-RPC App Server.

---

## Quick Start

### Build

```bash
cargo build --bin codex-web-server --release
```

### Run

```bash
./target/release/codex-web-server --port 3000
```

### Test

```bash
# Run all tests
cargo test -p codex-web-server

# Run with output
cargo test -p codex-web-server -- --nocapture
```

---

## Features

✅ **31 REST Endpoints** - Complete API coverage
✅ **SSE Event Streaming** - 27+ event types
✅ **Approval Flow** - SSE → REST pattern
✅ **MCP Integration** - Server status, OAuth login
✅ **Thread Management** - Create, resume, fork, rollback
✅ **Feedback Upload** - Sentry integration
✅ **26 Integration Tests** - Comprehensive test suite

---

## Architecture

```
Client (HTTP/SSE)
    ↓
Web Server (Axum)
    ↓ REST API
codex-core
    ↓
ThreadManager, AuthManager, ConfigService
```

**Key Components**:
- `src/handlers/` - REST endpoint implementations
- `src/event_stream.rs` - SSE event processing
- `src/state.rs` - Shared server state
- `tests/` - Integration tests

---

## API Endpoints

### Thread Management

```
POST   /api/v2/threads                    # Create thread
GET    /api/v2/threads                    # List threads
POST   /api/v2/threads/:id/resume         # Resume from rollout
POST   /api/v2/threads/:id/fork           # Fork thread
POST   /api/v2/threads/:id/archive        # Archive thread
POST   /api/v2/threads/:id/rollback       # Rollback to turn
```

### Turn Management

```
POST   /api/v2/threads/:id/turns          # Submit turn
POST   /api/v2/threads/:id/turns/interrupt # Interrupt turn
```

### Event Streaming

```
GET    /api/v2/threads/:id/events         # SSE stream
```

### MCP Servers

```
GET    /api/v2/mcp/servers                # List MCP status
POST   /api/v2/mcp/servers/refresh        # Refresh MCP config
POST   /api/v2/mcp/servers/:name/auth     # OAuth login
```

### Other

```
GET    /api/v2/config                     # Read config
PUT    /api/v2/config                     # Write config value
PATCH  /api/v2/config                     # Batch write config
POST   /api/v2/feedback                   # Upload feedback
POST   /api/v2/threads/:id/approvals/:approval_id  # Respond to approval
```

See [API.md](API.md) for complete reference.

---

## SSE Events

**Thread Events**:
- `thread/started`, `thread/tokenUsage/updated`, `thread/compacted`

**Turn Events**:
- `turn/started`, `turn/completed`, `turn/diff/updated`, `turn/plan/updated`

**Item Events**:
- `item/started`, `item/completed`, `item/agentMessage/delta`, `item/commandExecution/outputDelta`

**Approval Events**:
- `item/commandExecution/requestApproval`, `item/fileChange/requestApproval`

See [API.md#event-streaming-sse](API.md#event-streaming-sse) for details.

---

## Approval Flow

```
1. Server sends SSE event:
   event: item/commandExecution/requestApproval
   data: {"thread_id":"...","item_id":"...","reason":"..."}

2. Client responds via REST:
   POST /api/v2/threads/:thread_id/approvals/:item_id
   {"decision": "approve"}

3. Server continues turn execution
```

---

## Configuration

### Environment Variables

```bash
CODEX_AUTH_TOKEN=your-secret-token    # Auth token (required)
CODEX_HOME=/path/to/.codex            # Codex home dir (default: ~/.codex)
PORT=3000                             # Server port (default: 3000)
```

### Config File

`~/.codex/config.toml`:
```toml
model = "claude-sonnet-4-5"
approval_policy = "auto"
sandbox_mode = "workspace-write"

[mcp_servers.github]
transport = "streamable_http"
url = "https://api.github.com/mcp"
```

---

## Testing

### Unit Tests (26 tests)

```bash
cargo test -p codex-web-server
```

**Test Coverage**:
- ✅ Feedback upload (4 tests)
- ✅ Thread resume (5 tests)
- ✅ MCP servers (8 tests)
- ✅ SSE approval flow (9 tests)

See [tests/README.md](tests/README.md) for details.

---

### HTTP Integration Tests (Example)

```bash
# Future: Full HTTP tests with axum::test
cargo test -p codex-web-server --test http_integration
```

See [tests/suite/http_example.rs](tests/suite/http_example.rs) for implementation guide.

---

### Performance Tests

```bash
cargo bench -p codex-web-server
```

See [PERFORMANCE.md](PERFORMANCE.md) for benchmarking guide.

---

## Documentation

- **[API.md](API.md)** - Complete REST API reference
- **[MIGRATION.md](MIGRATION.md)** - Migrate from App Server
- **[IMPLEMENTATION_SUMMARY.md](IMPLEMENTATION_SUMMARY.md)** - Phase 4 & 5 summary
- **[PERFORMANCE.md](PERFORMANCE.md)** - Performance testing guide
- **[tests/README.md](tests/README.md)** - Test documentation

---

## Examples

### Create Thread and Submit Turn

```bash
# Create thread
THREAD_ID=$(curl -X POST http://localhost:3000/api/v2/threads \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"model":"claude-sonnet-4-5"}' | jq -r '.thread_id')

# Subscribe to events (in background)
curl -N http://localhost:3000/api/v2/threads/$THREAD_ID/events \
  -H "Authorization: Bearer $TOKEN" &

# Submit turn
curl -X POST http://localhost:3000/api/v2/threads/$THREAD_ID/turns \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"input":[{"type":"text","text":"Hello, Codex!"}]}'
```

---

### Resume Thread from Rollout

```bash
# Resume thread
curl -X POST http://localhost:3000/api/v2/threads/$THREAD_ID/resume \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{}'
```

---

### Upload Feedback

```bash
curl -X POST http://localhost:3000/api/v2/feedback \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "classification": "good_result",
    "reason": "Great response!",
    "thread_id": "'$THREAD_ID'",
    "include_logs": false
  }'
```

---

### List MCP Servers

```bash
curl http://localhost:3000/api/v2/mcp/servers?limit=20 \
  -H "Authorization: Bearer $TOKEN"
```

---

## Client Libraries

### JavaScript/TypeScript

```bash
npm install @codex/client
```

```typescript
import { CodexClient } from '@codex/client';

const client = new CodexClient({
  baseURL: 'http://localhost:3000',
  authToken: process.env.CODEX_AUTH_TOKEN
});

const thread = await client.threads.create({ model: 'claude-sonnet-4-5' });
await client.threads.submitTurn(thread.thread_id, {
  input: [{ type: 'text', text: 'Hello!' }]
});
```

---

### Python

```bash
pip install codex-client
```

```python
from codex import CodexClient

client = CodexClient(
    base_url='http://localhost:3000',
    auth_token=os.environ['CODEX_AUTH_TOKEN']
)

thread = client.threads.create(model='claude-sonnet-4-5')
client.threads.submit_turn(thread.thread_id, input=[
    {'type': 'text', 'text': 'Hello!'}
])
```

---

## Development

### Project Structure

```
web-server/
├── src/
│   ├── main.rs              # Server entry point
│   ├── lib.rs               # Library interface
│   ├── state.rs             # Shared state
│   ├── error.rs             # Error types
│   ├── middleware.rs        # Auth middleware
│   ├── event_stream.rs      # SSE processing
│   ├── approval_manager.rs  # Approval logic
│   └── handlers/
│       ├── mod.rs           # Main SSE handler
│       ├── threads.rs       # Thread endpoints
│       ├── turns.rs         # Turn endpoints
│       ├── mcp.rs           # MCP endpoints
│       ├── feedback.rs      # Feedback endpoint
│       └── ...
├── tests/
│   ├── all.rs               # Test entry point
│   ├── common/              # Test utilities
│   └── suite/               # Test modules
├── Cargo.toml
├── API.md
├── MIGRATION.md
├── PERFORMANCE.md
└── README.md
```

---

### Adding New Endpoints

1. Add handler in `src/handlers/{module}.rs`
2. Define request/response types
3. Add route in `src/main.rs`
4. Write tests in `tests/suite/{module}.rs`
5. Document in `API.md`

**Example**:
```rust
// src/handlers/example.rs
pub async fn my_endpoint(
    State(state): State<WebServerState>,
    Json(req): Json<MyRequest>,
) -> Result<Json<MyResponse>, ApiError> {
    // Implementation
}
```

---

## Dependencies

### Core

- `axum` - Web framework
- `tokio` - Async runtime
- `serde` / `serde_json` - Serialization
- `codex-core` - Codex business logic
- `codex-protocol` - Shared types
- `codex-feedback` - Sentry integration
- `codex-rmcp-client` - MCP client

### Dev

- `tempfile` - Test utilities
- `tower` - HTTP testing
- `criterion` - Benchmarking

---

## Troubleshooting

### SSE Connection Drops

**Symptom**: EventSource disconnects frequently

**Solution**: Ensure your HTTP client/proxy supports long-lived connections. Add retry logic.

---

### Approval Timeout

**Symptom**: `404 Not Found` when responding to approval

**Solution**: Approvals expire after 15 minutes. Ensure client responds promptly.

---

### Thread Not Found

**Symptom**: `404 Not Found` when resuming thread

**Solution**: Verify rollout file exists at `~/.codex/sessions/{thread_id}.jsonl`.

---

## Performance

**Targets**:
- Request latency: p95 < 20ms
- Throughput: > 2000 req/s
- SSE connections: > 1000 concurrent
- Memory: < 10 MB per thread

See [PERFORMANCE.md](PERFORMANCE.md) for benchmarks and optimization guide.

---

## Migration from App Server

See [MIGRATION.md](MIGRATION.md) for complete migration guide.

**Key Changes**:
- JSON-RPC → REST HTTP
- WebSocket notifications → SSE events
- Bidirectional RPC → SSE + REST response

---

## Contributing

1. Fork the repository
2. Create a feature branch
3. Write tests
4. Submit pull request

**Code Style**:
- Run `cargo fmt` before committing
- Run `cargo clippy` to check warnings
- Ensure all tests pass

---

## License

See LICENSE file in repository root.

---

## Support

- **Issues**: https://github.com/anthropics/codex/issues
- **Documentation**: https://docs.codex.anthropic.com
- **Email**: support@anthropic.com

---

## Changelog

### v2.0.0 (2026-01-18)

- Initial REST API release
- 31 endpoints covering full App Server functionality
- SSE event streaming (27+ event types)
- Approval flow (SSE → REST response)
- MCP OAuth login support
- Thread resume from rollout files
- Feedback upload integration
- 26 integration tests

---

## Acknowledgments

Built on top of:
- `codex-core` - Core Codex functionality
- `axum` - Web framework by Tower team
- `tokio` - Async runtime
- OpenAI Codex CLI architecture
