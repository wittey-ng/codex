# Codex Web Server REST API Reference

**Version**: v2
**Base URL**: `http://localhost:3000`
**Authentication**: Bearer token

---

## Table of Contents

1. [Authentication](#authentication)
2. [Thread Management](#thread-management)
3. [Turn Management](#turn-management)
4. [Event Streaming (SSE)](#event-streaming-sse)
5. [Configuration](#configuration)
6. [MCP Servers](#mcp-servers)
7. [Feedback](#feedback)
8. [Approvals](#approvals)
9. [Error Handling](#error-handling)

---

## Authentication

All API endpoints (except health checks) require Bearer token authentication.

**Header**:
```
Authorization: Bearer <token>
```

**Token Management**:
- Set via `CODEX_AUTH_TOKEN` environment variable
- Default: randomly generated at server startup

---

## Thread Management

### Create Thread

Create a new conversation thread.

**Endpoint**: `POST /api/v2/threads`

**Request Body**:
```json
{
  "cwd": "/path/to/project",      // optional
  "model": "claude-sonnet-4-5"    // optional
}
```

**Response**: `200 OK`
```json
{
  "thread_id": "019bcfb9-4ea6-72e0-b43d-6b7e26ff0daf",
  "model": "claude-sonnet-4-5"
}
```

**Example**:
```bash
curl -X POST http://localhost:3000/api/v2/threads \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{"cwd": "/Users/me/project", "model": "claude-sonnet-4-5"}'
```

---

### List Threads

Get all active threads.

**Endpoint**: `GET /api/v2/threads`

**Response**: `200 OK`
```json
{
  "threads": [
    {
      "thread_id": "019bcfb9-4ea6-72e0-b43d-6b7e26ff0daf",
      "model": "claude-sonnet-4-5",
      "created_at": "2026-01-18T12:00:00Z"
    }
  ]
}
```

---

### Resume Thread

Resume a thread from rollout file.

**Endpoint**: `POST /api/v2/threads/:thread_id/resume`

**Path Parameters**:
- `thread_id` (string, required): Thread ID (UUID format)

**Request Body**:
```json
{
  "config_overrides": {           // optional
    "model": "claude-opus-4-5"
  }
}
```

**Response**: `200 OK`
```json
{
  "success": true,
  "thread_id": "019bcfb9-4ea6-72e0-b43d-6b7e26ff0daf"
}
```

**Errors**:
- `404 Not Found`: Rollout file not found for thread
- `400 Bad Request`: Invalid thread ID format

**Notes**:
- Idempotent: Returns success if thread already active
- Loads history from `~/.codex/sessions/{thread_id}.jsonl`

---

### Archive Thread

Archive a thread (save to rollout file).

**Endpoint**: `POST /api/v2/threads/:thread_id/archive`

**Response**: `200 OK`
```json
{
  "success": true,
  "rollout_path": "/Users/me/.codex/sessions/019bcfb9-4ea6-72e0-b43d-6b7e26ff0daf.jsonl"
}
```

---

### Fork Thread

Fork a thread from a specific turn.

**Endpoint**: `POST /api/v2/threads/:thread_id/fork`

**Request Body**:
```json
{
  "turn_id": "turn-12345"         // optional
}
```

**Response**: `200 OK`
```json
{
  "new_thread_id": "019bcfb9-4ea6-72e0-b43d-6b7e26ff0daf"
}
```

---

### Rollback Thread

Rollback thread to a previous turn.

**Endpoint**: `POST /api/v2/threads/:thread_id/rollback`

**Request Body**:
```json
{
  "turn_id": "turn-12345"
}
```

**Response**: `200 OK`
```json
{
  "success": true,
  "current_turn_id": "turn-12345"
}
```

---

## Turn Management

### Submit Turn

Submit user input to start a new turn.

**Endpoint**: `POST /api/v2/threads/:thread_id/turns`

**Request Body**:
```json
{
  "input": [
    {
      "type": "text",
      "text": "Hello, Codex!"
    },
    {
      "type": "attachment",
      "attachment_id": "019bcfb9-4ea6-72e0-b43d-6b7e26ff0daf"
    }
  ]
}
```

**Response**: `200 OK`
```json
{
  "turn_id": "turn-12345"
}
```

**Input Types**:
- `text`: Plain text message
- `attachment`: File attachment (must be uploaded first)

---

### Interrupt Turn

Interrupt a running turn.

**Endpoint**: `POST /api/v2/threads/:thread_id/turns/interrupt`

**Response**: `200 OK`
```json
{
  "success": true
}
```

---

## Event Streaming (SSE)

### Subscribe to Events

Subscribe to Server-Sent Events for a thread.

**Endpoint**: `GET /api/v2/threads/:thread_id/events`

**Response**: `200 OK`
```
Content-Type: text/event-stream
```

**Event Format**:
```
event: <event-type>
data: <json-payload>

```

**Keepalive**: Every 10 seconds
```
: keepalive

```

---

### Event Types

#### Thread Events

**`thread/started`**
```json
{
  "thread_id": "019bcfb9-4ea6-72e0-b43d-6b7e26ff0daf",
  "model": "claude-sonnet-4-5"
}
```

**`thread/tokenUsage/updated`**
```json
{
  "input_tokens": 1000,
  "output_tokens": 500
}
```

**`thread/compacted`**
```json
{
  "removed_turns": 5,
  "saved_tokens": 10000
}
```

---

#### Turn Events

**`turn/started`**
```json
{
  "turn_id": "turn-12345",
  "thread_id": "019bcfb9-4ea6-72e0-b43d-6b7e26ff0daf"
}
```

**`turn/completed`**
```json
{
  "turn_id": "turn-12345",
  "status": "success"
}
```

**`turn/diff/updated`**
```json
{
  "turn_id": "turn-12345",
  "diff": "..."
}
```

**`turn/plan/updated`**
```json
{
  "turn_id": "turn-12345",
  "plan": "..."
}
```

---

#### Item Events

**`item/started`**
```json
{
  "item_id": "item-abc123",
  "item_type": "agent_message" | "tool_call"
}
```

**`item/completed`**
```json
{
  "item_id": "item-abc123"
}
```

**`item/agentMessage/delta`**
```json
{
  "item_id": "item-abc123",
  "delta": "Hello"
}
```

**`item/commandExecution/outputDelta`**
```json
{
  "item_id": "item-abc123",
  "output": "command output..."
}
```

**`item/fileChange/outputDelta`**
```json
{
  "item_id": "item-abc123",
  "file_path": "/path/to/file.rs",
  "change_type": "modified"
}
```

---

#### Approval Events

**`item/commandExecution/requestApproval`**
```json
{
  "thread_id": "019bcfb9-4ea6-72e0-b43d-6b7e26ff0daf",
  "turn_id": "turn-12345",
  "item_id": "item-abc123",
  "reason": "Execute dangerous command",
  "proposed_execpolicy_amendment": null
}
```

**Client Response**: `POST /api/v2/threads/:thread_id/approvals/:item_id`

**`item/fileChange/requestApproval`**
```json
{
  "thread_id": "019bcfb9-4ea6-72e0-b43d-6b7e26ff0daf",
  "turn_id": "turn-12345",
  "item_id": "item-abc123",
  "reason": "Modify critical files",
  "grant_root": "/path/to/project"
}
```

---

## Configuration

### Read Configuration

Get current configuration.

**Endpoint**: `GET /api/v2/config`

**Response**: `200 OK`
```json
{
  "model": "claude-sonnet-4-5",
  "approval_policy": "auto",
  "sandbox_mode": "workspace-write",
  "mcp_servers": { ... }
}
```

---

### Write Configuration Value

Update a single configuration value.

**Endpoint**: `PUT /api/v2/config`

**Request Body**:
```json
{
  "key": "model",
  "value": "claude-opus-4-5"
}
```

**Response**: `200 OK`
```json
{
  "success": true
}
```

---

### Batch Write Configuration

Update multiple configuration values.

**Endpoint**: `PATCH /api/v2/config`

**Request Body**:
```json
{
  "values": {
    "model": "claude-opus-4-5",
    "approval_policy": "auto"
  }
}
```

**Response**: `200 OK`
```json
{
  "success": true
}
```

---

## MCP Servers

### List MCP Server Status

Get status of all MCP servers with pagination.

**Endpoint**: `GET /api/v2/mcp/servers`

**Query Parameters**:
- `limit` (integer, optional): Max servers to return (1-100, default: 100)
- `cursor` (string, optional): Pagination cursor (offset as string)

**Response**: `200 OK`
```json
{
  "data": [
    {
      "name": "github",
      "tools": [
        {
          "name": "search_repos",
          "description": "Search GitHub repositories",
          "input_schema": { ... }
        }
      ],
      "resources": [ ... ],
      "resource_templates": [ ... ],
      "auth_status": "authenticated" | "unauthenticated" | "unsupported"
    }
  ],
  "next_cursor": "20"
}
```

**Example**:
```bash
# First page
curl http://localhost:3000/api/v2/mcp/servers?limit=20

# Second page
curl http://localhost:3000/api/v2/mcp/servers?limit=20&cursor=20
```

---

### Refresh MCP Servers

Refresh MCP server configuration.

**Endpoint**: `POST /api/v2/mcp/servers/refresh`

**Response**: `200 OK`
```json
{
  "success": true
}
```

---

### MCP OAuth Login

Initiate OAuth login for an MCP server.

**Endpoint**: `POST /api/v2/mcp/servers/:name/auth`

**Path Parameters**:
- `name` (string, required): MCP server name

**Response**: `200 OK`
```json
{
  "auth_url": "https://oauth.provider.com/authorize?..."
}
```

**OAuth Flow**:
1. Client receives `auth_url`
2. User completes OAuth in browser
3. Server completes OAuth in background
4. SSE event `mcpServer/oauthLogin/completed` sent

**SSE Event**:
```
event: mcpServer/oauthLogin/completed
data: {"name":"github","success":true,"error":null}
```

**Errors**:
- `404 Not Found`: MCP server not found
- `400 Bad Request`: OAuth not supported (only for streamable_http transport)

---

## Feedback

### Upload Feedback

Submit user feedback.

**Endpoint**: `POST /api/v2/feedback`

**Request Body**:
```json
{
  "classification": "bug" | "bad_result" | "good_result",
  "reason": "Description of feedback",
  "thread_id": "019bcfb9-4ea6-72e0-b43d-6b7e26ff0daf",  // optional
  "include_logs": false
}
```

**Response**: `201 Created`
```json
{
  "success": true,
  "thread_id": "019bcfb9-4ea6-72e0-b43d-6b7e26ff0daf"
}
```

**Notes**:
- Fire-and-forget: Returns immediately while upload happens in background
- If `thread_id` provided, includes rollout file in feedback
- Uploads to Sentry via `codex-feedback`

**Errors**:
- `400 Bad Request`: Empty classification

---

## Approvals

### Respond to Approval Request

Approve or decline a command execution or file change.

**Endpoint**: `POST /api/v2/threads/:thread_id/approvals/:approval_id`

**Path Parameters**:
- `thread_id` (string, required): Thread ID
- `approval_id` (string, required): Approval item ID (from SSE event)

**Request Body**:
```json
{
  "decision": "approve" | "decline",
  "amendments": {                        // optional
    "execpolicy": { ... }
  }
}
```

**Response**: `200 OK`
```json
{
  "success": true
}
```

**Flow**:
1. Server sends SSE event `item/commandExecution/requestApproval`
2. Client responds via this endpoint
3. Server continues turn execution with approval decision

**Errors**:
- `404 Not Found`: Approval request not found or expired (15 min timeout)
- `400 Bad Request`: Invalid decision value

---

## Error Handling

### Error Response Format

All errors return this structure:

```json
{
  "error": {
    "code": "ERROR_CODE",
    "message": "Human-readable error message"
  }
}
```

### HTTP Status Codes

- `200 OK`: Success
- `201 Created`: Resource created (feedback)
- `400 Bad Request`: Invalid request (malformed JSON, invalid parameters)
- `401 Unauthorized`: Missing or invalid auth token
- `404 Not Found`: Resource not found (thread, approval, file)
- `500 Internal Server Error`: Server error

### Common Errors

**Invalid Thread ID**:
```json
{
  "error": {
    "code": "INVALID_REQUEST",
    "message": "Invalid thread ID"
  }
}
```

**Thread Not Found**:
```json
{
  "error": {
    "code": "THREAD_NOT_FOUND",
    "message": "Thread not found: 019bcfb9-4ea6-72e0-b43d-6b7e26ff0daf"
  }
}
```

**Rollout File Not Found**:
```json
{
  "error": {
    "code": "NOT_FOUND",
    "message": "Rollout file not found for thread: 019bcfb9-4ea6-72e0-b43d-6b7e26ff0daf"
  }
}
```

---

## Rate Limiting

Currently no rate limiting is enforced. Future versions may implement:
- Per-token rate limits
- Per-endpoint rate limits
- Configurable via `rate_limit` config key

---

## Versioning

**Current Version**: v2

API endpoints are versioned via URL path: `/api/v2/...`

**Backward Compatibility**:
- v1 endpoints (`/api/v1/attachments`) remain available
- Breaking changes will increment version number
- Deprecation notices sent via SSE (`deprecationNotice` event)

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

const thread = await client.threads.create({
  model: 'claude-sonnet-4-5'
});

await client.threads.submitTurn(thread.thread_id, {
  input: [{ type: 'text', text: 'Hello!' }]
});

// Subscribe to events
for await (const event of client.threads.streamEvents(thread.thread_id)) {
  console.log(event);
}
```

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

# Subscribe to events
for event in client.threads.stream_events(thread.thread_id):
    print(event)
```

---

## Example Workflows

### Complete Turn with Approval

```bash
# 1. Create thread
THREAD_ID=$(curl -X POST http://localhost:3000/api/v2/threads \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"model":"claude-sonnet-4-5"}' | jq -r '.thread_id')

# 2. Start SSE stream (in background)
curl -N http://localhost:3000/api/v2/threads/$THREAD_ID/events \
  -H "Authorization: Bearer $TOKEN" &

# 3. Submit turn
curl -X POST http://localhost:3000/api/v2/threads/$THREAD_ID/turns \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"input":[{"type":"text","text":"Delete all files in /tmp"}]}'

# 4. Receive approval request via SSE
# event: item/commandExecution/requestApproval
# data: {"item_id":"item-abc123",...}

# 5. Respond to approval
curl -X POST http://localhost:3000/api/v2/threads/$THREAD_ID/approvals/item-abc123 \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"decision":"approve"}'

# 6. Turn continues and completes
```

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

---

## Support

- **Issues**: https://github.com/anthropics/codex/issues
- **Documentation**: https://docs.codex.anthropic.com
- **Email**: support@anthropic.com
