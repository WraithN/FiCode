# Web 服务设计文档

> 为 ficode 添加基于 JSON-RPC + SSE 的 Web API 服务，支持指令执行和 Agent 对话。

---

## 1. 背景与目标

### 1.1 现状

ficode 目前仅支持终端 REPL 和单命令模式，所有交互必须通过命令行完成。用户无法通过 HTTP API 远程调用指令或与 Agent 对话。

### 1.2 目标

1. 新增 `ficode server` 子命令启动 Web 服务
2. 提供 JSON-RPC 2.0 接口用于执行指令（`/model`, `/init` 等）和查询状态
3. 提供 SSE (Server-Sent Events) 接口用于与 Agent 进行多轮流式对话
4. 支持可选的 Bearer Token 认证和可配置的 CORS
5. 会话状态通过内存管理，支持断连恢复

---

## 2. 架构设计

### 2.1 模块划分

```
src/
├── server/
│   ├── mod.rs          # 模块入口，导出 Server
│   ├── server.rs       # Axum 应用、路由、中间件、启动逻辑
│   ├── rpc.rs          # JSON-RPC 协议类型与请求分发器
│   ├── sse.rs          # SSE 流封装
│   └── session.rs      # HTTP 会话管理（session_id ↔ LoopState）
├── utils/
│   └── cli.rs          # 新增 server 子命令
├── entry.rs            # run() 中增加 server 分支
├── config/
│   └── models.rs       # 新增 server 配置字段
└── Cargo.toml          # 新增 axum, tokio-stream, tower-http
```

### 2.2 技术栈

| 依赖 | 用途 |
|------|------|
| `axum` | HTTP 框架、路由、中间件 |
| `tokio-stream` | SSE 流处理 |
| `tower-http` | CORS 中间件 |

### 2.3 文件职责

#### `src/server/server.rs`

- 构建 Axum Router：挂载 `/rpc` (POST) 和 `/chat` (POST+SSE)
- 配置 Tower 中间件：CORS、Auth（可选）
- 绑定 TCP 端口（默认 4040）
- 持有 `Provider`、`Config`、`HttpSessionManager` 的共享引用

#### `src/server/rpc.rs`

- `JsonRpcRequest` / `JsonRpcResponse` 结构体
- `handle_rpc()`：根据 `method` 字段分发到具体处理器
- 已支持的 methods：`execute`、`list_models`、`get_status`

#### `src/server/sse.rs`

- `SseStream`：包装 `mpsc::Receiver<SseEvent>` 为 Axum 的 `Sse` response
- `SseEvent` 枚举：`Message`、`ToolUse`、`ToolResult`、`Error`、`Done`
- 将 `stream_message` 的 `Chunk` 实时转换为 SSE event

#### `src/server/session.rs`

- `HttpSessionManager`：线程安全的 `HashMap<String, (LoopState, Instant)>`
- `create()`：生成 ULID，创建新的 LoopState
- `get(id)`：读取会话并刷新活动时间戳
- `save(id, state)`：保存/更新会话状态
- `cleanup()`：删除超过 30 分钟未活动的会话

---

## 3. API 设计

### 3.1 JSON-RPC 协议

**请求格式：**
```json
{
  "jsonrpc": "2.0",
  "method": "execute",
  "params": { "command": "/model gpt-4o" },
  "id": 1
}
```

**成功响应：**
```json
{
  "jsonrpc": "2.0",
  "result": { "message": "✅ 已切换模型: gpt-4o" },
  "id": 1
}
```

**错误响应：**
```json
{
  "jsonrpc": "2.0",
  "error": { "code": -32602, "message": "没有此模型: invalid-model" },
  "id": 1
}
```

### 3.2 Method: `execute`

执行 slash 指令或其他直接命令。

**Request：**
```json
{
  "method": "execute",
  "params": { "command": "/init" },
  "id": 1
}
```

**Response：**
```json
{
  "result": {
    "success": true,
    "message": "✅ AGENTS.md 已生成: /path/to/AGENTS.md"
  }
}
```

### 3.3 Method: `list_models`

获取配置中所有可用模型。

**Response：**
```json
{
  "result": {
    "models": [
      { "key": "gpt-4o", "name": "OpenAI GPT-4o", "context": 128000, "output": 4096 },
      { "key": "claude-3-7-sonnet", "name": "Anthropic Claude 3.7", "context": 200000, "output": 65536 }
    ]
  }
}
```

### 3.4 Method: `get_status`

获取服务状态。

**Response：**
```json
{
  "result": {
    "status": "running",
    "version": "0.1.0",
    "current_model": "gpt-4o",
    "active_sessions": 3
  }
}
```

### 3.5 `/chat` 端点（SSE）

**Request：**
```http
POST /chat
Content-Type: application/json

{
  "session_id": "",
  "message": "帮我写一个 Rust Hello World"
}
```

- `session_id` 为空时创建新会话
- `session_id` 存在时恢复该会话上下文

**SSE Response：**
```text
HTTP/1.1 200 OK
Content-Type: text/event-stream
Cache-Control: no-cache

event: message
data: {"type":"text","content":"我来帮你写"}

event: message
data: {"type":"text","content":"一个 Rust Hello World"}

event: tool_use
data: {"type":"tool_use","id":"tool-1","name":"write","arguments":{...}}

event: tool_result
data: {"type":"tool_result","tool_use_id":"tool-1","content":"文件已写入"}

event: done
data: {"type":"done","session_id":"01HWXYZ..."}
```

**Event Types：**

| Event | 说明 |
|-------|------|
| `message` | LLM 返回的文本片段 |
| `tool_use` | LLM 请求调用工具 |
| `tool_result` | 工具执行结果 |
| `error` | 错误信息 |
| `done` | 对话结束，携带最终 `session_id` |

---

## 4. 数据流

### 4.1 `/rpc` — `execute` 数据流

```
Client POST /rpc
{ "method": "execute", "params": { "command": "/model gpt-4o" }, "id": 1 }
        │
        ▼
┌─────────────────┐
│ Axum Router     │  匹配 POST /rpc
│ └─ auth mw      │  校验 Bearer Token（如果配置了）
│ └─ cors mw      │  校验 Origin
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ rpc::handle_rpc │  解析 JsonRpcRequest
│ ├─ dispatch     │  匹配 method = "execute"
│ └─ params解析   │  提取 command = "/model gpt-4o"
└────────┬────────┘
         │
         ▼
┌─────────────────────────┐
│ slash::parse("/model")  │  → SlashCommand::Model(Some("gpt-4o"))
│ SlashCommandHandler     │
│ ├─ provider.set_model() │
│ └─ 返回结果             │
└────────┬────────────────┘
         │
         ▼
┌─────────────────┐
│ JsonRpcResponse │  { result: { message: "✅..." }, id: 1 }
│ serialize       │
└────────┬────────┘
         │
         ▼
    HTTP 200 OK
```

### 4.2 `/chat` — SSE 数据流

```
Client POST /chat
{ "session_id": "", "message": "帮我写Hello World" }
        │
        ▼
┌─────────────────────────┐
│ server::handle_chat     │
│ ├─ session_id为空?      │
│ │   ├─ 是 → 创建新session│  生成 ULID，创建 LoopState
│ │   └─ 否 → 查找已有     │  HttpSessionManager.get(session_id)
│ ├─ push user message    │  LoopState.messages.push(...)
│ └─ 启动 agent_loop      │  在独立 tokio task 中运行
└────────┬────────────────┘
         │
         ▼
┌─────────────────────────┐
│ sse::SseStream          │  创建 mpsc channel
│ ├─ sender → agent_loop  │  每个 chunk 写入 channel
│ └─ receiver → SSE       │  消费者转为 text/event-stream
└────────┬────────────────┘
         │
         ▼
┌─────────────────────────┐
│ 客户端收到 SSE          │
│ event: message          │
│ data: {"type":"text"...}│
│                         │
│ event: done             │
│ data: {"session_id":"x"}│
└─────────────────────────┘
```

**后台 agent_loop 执行：**
1. `stream_message()` 流式获取 LLM 输出
2. 每个 chunk 通过 `mpsc::Sender` 推送到 SSE 流
3. ToolUse 时：执行工具 → 发送 `tool_result` event → 继续下一轮
4. 对话结束时：发送 `done` event（携带 `session_id`），保存 `LoopState` 到 `HttpSessionManager`

---

## 5. 配置与安全

### 5.1 配置扩展

`config.json` / `config.jsonc` 新增 `server` 字段：

```json
{
  "server": {
    "port": 4040,
    "api_token": "{env:FICODE_API_TOKEN}",
    "allowed_origins": ["http://localhost:3000", "http://localhost:5173"]
  }
}
```

- `port`：监听端口，默认 `4040`
- `api_token`：可选。如果存在，请求必须带 `Authorization: Bearer <token>`
- `allowed_origins`：可选。如果存在，CORS 只允许这些 Origin；不存在则允许 `*`

### 5.2 认证中间件

```rust
async fn auth_middleware(
    headers: HeaderMap,
    config: Arc<RwLock<Config>>,
    next: Next,
) -> Response {
    let cfg = config.read().unwrap();
    if let Some(expected_token) = &cfg.server.api_token {
        let auth = headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if !auth.starts_with("Bearer ") || &auth[7..] != expected_token {
            return (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
        }
    }
    next.run(request).await
}
```

### 5.3 CORS 中间件

```rust
fn cors_layer(config: Arc<RwLock<Config>>) -> CorsLayer {
    let cfg = config.read().unwrap();
    if let Some(origins) = &cfg.server.allowed_origins {
        let mut layer = CorsLayer::new();
        for origin in origins {
            layer = layer.allow_origin(origin.parse::<HeaderValue>().unwrap());
        }
        layer
            .allow_methods([Method::GET, Method::POST])
            .allow_headers([AUTHORIZATION, CONTENT_TYPE])
    } else {
        CorsLayer::permissive()
    }
}
```

### 5.4 会话清理策略

`HttpSessionManager` 每 5 分钟扫描一次，删除超过 30 分钟未活动的会话，防止内存泄漏。

---

## 6. 错误处理

| 场景 | HTTP 状态码 | 返回 |
|------|-------------|------|
| JSON-RPC method 不存在 | 200 | `{"error": {"code": -32601, "message": "Method not found"}}` |
| 参数无效 | 200 | `{"error": {"code": -32602, "message": "Invalid params"}}` |
| 认证失败 | 401 | `Unauthorized` |
| CORS 拒绝 | 403 | `CORS error` |
| 服务端内部错误 | 200 | `{"error": {"code": -32603, "message": "Internal error"}}` |
| Session 不存在 | 200 (SSE) | `event: error\ndata: {"message": "Session not found"}` |

---

## 7. 实现顺序

1. **修改 `Cargo.toml`** — 添加 `axum`、`tokio-stream`、`tower-http`
2. **修改 `src/config/models.rs`** — 新增 `ServerConfig` 结构体
3. **修改 `src/utils/cli.rs`** — 新增 `server` 子命令
4. **修改 `src/entry.rs`** — `run()` 中增加 `server` 分支
5. **创建 `src/server/session.rs`** — `HttpSessionManager`
6. **创建 `src/server/rpc.rs`** — JSON-RPC 类型和分发器
7. **创建 `src/server/sse.rs`** — SSE 流封装
8. **创建 `src/server/server.rs`** — Axum 路由和中间件
9. **创建 `src/server/mod.rs`** — 模块导出
10. **运行测试** — `cargo test`、`cargo clippy`

---

## 8. 兼容性说明

- **不影响现有 CLI 功能**：`ficode -i`、`-c`、`-s` 等完全保留
- **配置文件向后兼容**：不配置 `server` 字段时服务不可用，但 CLI 正常
- **二进制名变更**：从 `fi-code` 改为 `ficode`（Cargo.toml `[[bin]]` name）
