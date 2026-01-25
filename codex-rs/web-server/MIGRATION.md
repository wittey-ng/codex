# Migration Guide: App Server → Web Server

This guide helps you migrate from the JSON-RPC App Server to the REST Web Server.

---

## Table of Contents

1. [Overview](#overview)
2. [Key Differences](#key-differences)
3. [Endpoint Mapping](#endpoint-mapping)
4. [Event Streaming Changes](#event-streaming-changes)
5. [Authentication](#authentication)
6. [Code Examples](#code-examples)
7. [Breaking Changes](#breaking-changes)
8. [Migration Checklist](#migration-checklist)

---

## Overview

### Why Migrate?

**App Server (JSON-RPC)**:
- ❌ Complex JSON-RPC protocol
- ❌ Bidirectional RPC (server can call client)
- ❌ Custom transport layer
- ❌ IDE-specific integration

**Web Server (REST)**:
- ✅ Standard HTTP REST API
- ✅ Server-Sent Events (SSE) for streaming
- ✅ Language-agnostic (curl, fetch, axios, etc.)
- ✅ Works with any HTTP client

### Migration Path

```
JSON-RPC method call → HTTP REST endpoint
JSON-RPC notification → SSE event
Approval RPC → SSE event + REST response
```

---

## Key Differences

### 1. Request/Response Pattern

**App Server (JSON-RPC)**:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "thread/start",
  "params": {
    "model": "claude-sonnet-4-5"
  }
}
```

**Web Server (REST)**:
```bash
POST /api/v2/threads
Content-Type: application/json

{
  "model": "claude-sonnet-4-5"
}
```

---

### 2. Event Streaming

**App Server (JSON-RPC)**:
```json
{
  "jsonrpc": "2.0",
  "method": "turn/started",
  "params": {
    "turn_id": "turn-123"
  }
}
```

**Web Server (SSE)**:
```
event: turn/started
data: {"turn_id":"turn-123"}

```

---

### 3. Approval Flow

**App Server (Bidirectional RPC)**:
```
Server → Client: approval/request (method call)
Client → Server: approval/response (result)
```

**Web Server (SSE + REST)**:
```
Server → Client: SSE event (item/commandExecution/requestApproval)
Client → Server: POST /api/v2/threads/:id/approvals/:approval_id
```

---

## Endpoint Mapping

### Thread Management

| App Server Method | Web Server Endpoint | Notes |
|------------------|---------------------|-------|
| `thread/start` | `POST /api/v2/threads` | Renamed params |
| `thread/list` | `GET /api/v2/threads` | Same response |
| `thread/loaded/list` | `GET /api/v2/threads/loaded` | New endpoint |
| `thread/resume` | `POST /api/v2/threads/:id/resume` | Path param |
| `thread/fork` | `POST /api/v2/threads/:id/fork` | Path param |
| `thread/archive` | `POST /api/v2/threads/:id/archive` | Path param |
| `thread/rollback` | `POST /api/v2/threads/:id/rollback` | Path param |

**Migration Example**:

**Before (JSON-RPC)**:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "thread/start",
  "params": {
    "cwd": "/project",
    "model": "claude-sonnet-4-5"
  }
}
```

**After (REST)**:
```bash
POST /api/v2/threads
{
  "cwd": "/project",
  "model": "claude-sonnet-4-5"
}
```

---

### Turn Management

| App Server Method | Web Server Endpoint | Notes |
|------------------|---------------------|-------|
| `turn/start` | `POST /api/v2/threads/:id/turns` | Path param |
| `turn/interrupt` | `POST /api/v2/threads/:id/turns/interrupt` | New endpoint |

**Migration Example**:

**Before (JSON-RPC)**:
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "turn/start",
  "params": {
    "conversation_id": "thread-123",
    "items": [
      {"type": "text", "text": "Hello"}
    ]
  }
}
```

**After (REST)**:
```bash
POST /api/v2/threads/thread-123/turns
{
  "input": [
    {"type": "text", "text": "Hello"}
  ]
}
```

**Note**: `conversation_id` → path parameter, `items` → `input`

---

### Configuration

| App Server Method | Web Server Endpoint | Notes |
|------------------|---------------------|-------|
| `config/read` | `GET /api/v2/config` | Same |
| `config/value/write` | `PUT /api/v2/config` | Same |
| `config/batchWrite` | `PATCH /api/v2/config` | Same |
| `configRequirements/read` | `GET /api/v2/config/requirements` | Same |

---

### MCP Servers

| App Server Method | Web Server Endpoint | Notes |
|------------------|---------------------|-------|
| `mcpServerStatus/list` | `GET /api/v2/mcp/servers` | Query params |
| `config/mcpServer/reload` | `POST /api/v2/mcp/servers/refresh` | Renamed |
| `mcpServer/oauth/login` | `POST /api/v2/mcp/servers/:name/auth` | Path param |

**Migration Example**:

**Before (JSON-RPC)**:
```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "mcpServerStatus/list",
  "params": {
    "limit": 20,
    "cursor": "10"
  }
}
```

**After (REST)**:
```bash
GET /api/v2/mcp/servers?limit=20&cursor=10
```

---

### Authentication

| App Server Method | Web Server Endpoint | Notes |
|------------------|---------------------|-------|
| `account/login/start` | `POST /api/v2/auth/login` | Same |
| `account/login/cancel` | `POST /api/v2/auth/login/cancel` | Same |
| `account/logout` | `POST /api/v2/auth/logout` | Same |
| `account/read` | `GET /api/v2/auth/account` | Same |
| `account/rateLimits/read` | `GET /api/v2/auth/rate-limits` | Same |

---

### Other

| App Server Method | Web Server Endpoint | Notes |
|------------------|---------------------|-------|
| `feedback/upload` | `POST /api/v2/feedback` | New structure |
| `command/exec` | `POST /api/v2/commands/exec` | One-off commands |
| `review/start` | `POST /api/v2/threads/:id/review` | Path param |
| `model/list` | `GET /api/v2/models` | Same |
| `skills/list` | `GET /api/v2/skills` | Same |
| `skills/config/write` | `PUT /api/v2/skills/:name/config` | Path param |

---

## Event Streaming Changes

### Event Format

**App Server (JSON-RPC Notification)**:
```json
{
  "jsonrpc": "2.0",
  "method": "thread/started",
  "params": {
    "thread_id": "thread-123",
    "model": "claude-sonnet-4-5"
  }
}
```

**Web Server (SSE)**:
```
event: thread/started
data: {"thread_id":"thread-123","model":"claude-sonnet-4-5"}

```

### Event Type Mapping

| App Server Notification | Web Server SSE Event | Changes |
|------------------------|---------------------|---------|
| `thread/started` | `thread/started` | Same |
| `turn/started` | `turn/started` | Same |
| `turn/completed` | `turn/completed` | Same |
| `item/started` | `item/started` | Same |
| `item/agentMessage/delta` | `item/agentMessage/delta` | Same |
| `approval/request` | `item/commandExecution/requestApproval` | New type |
| `patch/approval/request` | `item/fileChange/requestApproval` | New type |

### Client Code Changes

**Before (JSON-RPC WebSocket)**:
```javascript
const socket = new WebSocket('ws://localhost:3000');

socket.on('message', (data) => {
  const msg = JSON.parse(data);
  if (msg.method === 'turn/started') {
    console.log('Turn started:', msg.params);
  }
});
```

**After (SSE)**:
```javascript
const eventSource = new EventSource(
  'http://localhost:3000/api/v2/threads/thread-123/events'
);

eventSource.addEventListener('turn/started', (event) => {
  const data = JSON.parse(event.data);
  console.log('Turn started:', data);
});
```

---

## Authentication

### App Server

**WebSocket Handshake**:
```
Authorization: Bearer <token>
```

### Web Server

**HTTP Header (All Requests)**:
```
Authorization: Bearer <token>
```

**Same token format**, just applied to HTTP instead of WebSocket.

---

## Code Examples

### JavaScript/TypeScript

**Before (App Server)**:
```typescript
import { AppServerClient } from '@codex/app-server-client';

const client = new AppServerClient('ws://localhost:3000');

// Create thread
const thread = await client.call('thread/start', {
  model: 'claude-sonnet-4-5'
});

// Listen for notifications
client.on('notification', (method, params) => {
  if (method === 'turn/started') {
    console.log(params);
  }
});

// Submit turn
await client.call('turn/start', {
  conversation_id: thread.thread_id,
  items: [{ type: 'text', text: 'Hello' }]
});
```

**After (Web Server)**:
```typescript
import { CodexClient } from '@codex/client';

const client = new CodexClient({
  baseURL: 'http://localhost:3000',
  authToken: process.env.CODEX_AUTH_TOKEN
});

// Create thread
const thread = await client.threads.create({
  model: 'claude-sonnet-4-5'
});

// Listen for events
for await (const event of client.threads.streamEvents(thread.thread_id)) {
  if (event.type === 'turn/started') {
    console.log(event.data);
  }
}

// Submit turn
await client.threads.submitTurn(thread.thread_id, {
  input: [{ type: 'text', text: 'Hello' }]
});
```

---

### Python

**Before (App Server)**:
```python
from codex_app_server import AppServerClient

client = AppServerClient('ws://localhost:3000')

# Create thread
thread = client.call('thread/start', {
    'model': 'claude-sonnet-4-5'
})

# Listen for notifications
@client.on_notification
def handle_notification(method, params):
    if method == 'turn/started':
        print(params)

# Submit turn
client.call('turn/start', {
    'conversation_id': thread['thread_id'],
    'items': [{'type': 'text', 'text': 'Hello'}]
})
```

**After (Web Server)**:
```python
from codex import CodexClient

client = CodexClient(
    base_url='http://localhost:3000',
    auth_token=os.environ['CODEX_AUTH_TOKEN']
)

# Create thread
thread = client.threads.create(model='claude-sonnet-4-5')

# Listen for events
for event in client.threads.stream_events(thread.thread_id):
    if event.type == 'turn/started':
        print(event.data)

# Submit turn
client.threads.submit_turn(thread.thread_id, input=[
    {'type': 'text', 'text': 'Hello'}
])
```

---

## Breaking Changes

### 1. Parameter Naming

| Old (App Server) | New (Web Server) | Scope |
|-----------------|------------------|-------|
| `conversation_id` | `thread_id` (path param) | All turn endpoints |
| `items` | `input` | Submit turn |
| `conversation_id` (param) | `:thread_id` (path) | Most endpoints |

### 2. Approval Flow

**Old**: Server calls client method, client returns result

**New**: Server sends SSE event, client POSTs to REST endpoint

**Migration**:
```javascript
// Before
client.on_method('approval/request', async (params) => {
  return { decision: 'approve' };
});

// After
eventSource.addEventListener('item/commandExecution/requestApproval', async (event) => {
  const data = JSON.parse(event.data);
  await fetch(`/api/v2/threads/${data.thread_id}/approvals/${data.item_id}`, {
    method: 'POST',
    body: JSON.stringify({ decision: 'approve' })
  });
});
```

### 3. Response Structures

Some responses have been simplified:

**Before**:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "thread_id": "thread-123",
    "model": "claude-sonnet-4-5"
  }
}
```

**After**:
```json
{
  "thread_id": "thread-123",
  "model": "claude-sonnet-4-5"
}
```

No `jsonrpc`, `id`, or `result` wrapper.

---

## Migration Checklist

### Pre-Migration

- [ ] Review API documentation (`API.md`)
- [ ] Identify all JSON-RPC method calls in your codebase
- [ ] List all notification handlers
- [ ] Check for bidirectional RPC patterns (approvals)
- [ ] Plan downtime or parallel deployment strategy

### Code Changes

- [ ] Replace JSON-RPC client with HTTP/REST client
- [ ] Update all method calls to REST endpoints
- [ ] Convert path parameters (`conversation_id` → `:thread_id`)
- [ ] Rename request fields (`items` → `input`)
- [ ] Replace WebSocket/RPC notifications with SSE listeners
- [ ] Implement approval response via REST POST
- [ ] Update authentication (same token, different transport)
- [ ] Handle new error response format

### Testing

- [ ] Test thread creation and management
- [ ] Test turn submission and streaming
- [ ] Test approval flow (SSE → REST)
- [ ] Test MCP server status and OAuth
- [ ] Test configuration updates
- [ ] Test feedback upload
- [ ] Load testing (ensure performance parity)

### Deployment

- [ ] Update environment variables (if needed)
- [ ] Deploy Web Server alongside App Server (parallel)
- [ ] Gradually migrate clients
- [ ] Monitor error rates and performance
- [ ] Deprecate App Server once migration complete

---

## Troubleshooting

### Issue: SSE Connection Drops

**Symptom**: EventSource disconnects frequently

**Solution**: Ensure your HTTP client/proxy supports long-lived connections. Add retry logic:

```javascript
const eventSource = new EventSource(url);

eventSource.onerror = () => {
  console.log('Reconnecting...');
  setTimeout(() => {
    eventSource.close();
    // Reconnect
  }, 1000);
};
```

---

### Issue: Approval Timeout

**Symptom**: `404 Not Found` when responding to approval

**Solution**: Approvals expire after 15 minutes. Ensure client responds promptly. Check `created_at` timestamp.

---

### Issue: Thread Not Found

**Symptom**: `404 Not Found` when resuming thread

**Solution**: Verify rollout file exists at `~/.codex/sessions/{thread_id}.jsonl`. Check thread ID format (must be valid UUID).

---

## Support

### Resources

- **REST API Reference**: `API.md`
- **Example Code**: `examples/` directory
- **Tests**: `web-server/tests/` for working examples

### Getting Help

- **GitHub Issues**: https://github.com/anthropics/codex/issues
- **Discord**: https://discord.gg/codex
- **Email**: support@anthropic.com

---

## FAQ

**Q: Can I use both App Server and Web Server simultaneously?**

A: Yes, they can run on different ports. Useful for gradual migration.

**Q: Do I need to change my authentication token?**

A: No, the same token works for both. Just change how you send it (HTTP header instead of WebSocket header).

**Q: Will my existing rollout files work with Web Server?**

A: Yes, rollout file format is identical. Resume works the same way.

**Q: Is there a performance difference?**

A: Web Server is designed for < 10% overhead vs App Server. See benchmarks in `PERFORMANCE.md`.

**Q: Can I still use MCP servers?**

A: Yes, MCP integration is identical. Same config, same servers, same tools.

---

## Next Steps

1. Read `API.md` for complete endpoint reference
2. Run example code in `examples/migration/`
3. Start with non-critical client for testing
4. Monitor logs and metrics during migration
5. Provide feedback to help improve the migration experience!
