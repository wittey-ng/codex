# Codex Web Server API（v1）

## 基本信息

- 默认地址：`http://127.0.0.1:8080`
- 绑定地址：可通过环境变量 `CODEX_WEB_BIND_ADDR` 覆盖
- OpenAPI 文档：`/swagger-ui` 和 `/api-docs/openapi.json`

## 认证

除 `/health` 外，其余接口都需要 Bearer Token：

```
Authorization: Bearer <token>
```

Token 来源：
- 环境变量 `CODEX_WEB_TOKEN`
- 如果未设置，服务启动时会在日志里打印一次随机生成的 token

## 统一错误响应

```
{
  "error": "错误信息",
  "status": 400
}
```

## 接口列表

### GET /health

- 认证：不需要
- 返回：

```json
{ "status": "ok" }
```

---

### POST /api/v1/threads

- 认证：需要
- Content-Type：`application/json`
- 请求体：

```json
{
  "cwd": "/path/to/project",
  "model": "claude-sonnet-4-5"
}
```

- 参数说明：
  - `cwd`（可选，string）：线程工作目录
  - `model`（可选，string）：模型名称

- 返回：

```json
{
  "thread_id": "019bcfb9-4ea6-72e0-b43d-6b7e26ff0daf",
  "model": "claude-sonnet-4-5"
}
```

---

### POST /api/v1/threads/{thread_id}/turns

- 认证：需要
- Content-Type：`application/json`
- Path 参数：
  - `thread_id`（string）：线程 ID

- 请求体：

```json
{
  "input": [
    { "type": "text", "text": "Hello, Codex!" },
    { "type": "attachment", "attachment_id": "019bcfb9-4ea6-72e0-b43d-6b7e26ff0daf" }
  ]
}
```

- `input` 项格式：
  - 文本：`{ "type": "text", "text": "..." }`
  - 附件：`{ "type": "attachment", "attachment_id": "<uuid>" }`

- 返回：

```json
{ "turn_id": "turn-12345" }
```

---

### GET /api/v1/threads/{thread_id}/events

- 认证：需要
- Path 参数：
  - `thread_id`（string）：线程 ID
- 返回：SSE（`text/event-stream`）
  - `data:` 为 JSON 字符串
  - 服务端每 10 秒发送一次 keepalive

---

### POST /api/v1/attachments

- 认证：需要
- Content-Type：`multipart/form-data`
- 请求体：
  - 仅处理**第一个**文件字段作为上传内容

- 返回：

```json
{
  "attachment_id": "019bcfb9-4ea6-72e0-b43d-6b7e26ff0daf",
  "filename": "image.png",
  "size": 1024
}
```

- 说明：
  - 单文件最大 100MB（服务端逻辑限制）

---

### GET /api/v1/attachments/{id}

- 认证：需要
- Path 参数：
  - `id`（string/uuid）：附件 ID
- 返回：文件二进制流
  - `Content-Type` 来自上传时的 MIME
  - `Content-Disposition` 为附件下载

