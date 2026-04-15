# Session-Message-Part 持久化系统设计文档

> 版本：2025-04-15  
> 范围：完整实现 Session 持久化、多会话管理、JSONL 存储与恢复

---

## 1. 设计目标

- **持久化**：用户退出 CLI 后，会话内容自动保存，下次启动可恢复。
- **多会话**：支持创建多个会话、列出历史会话、切换恢复任意会话。
- **可恢复性**：流式生成的对话支持从中断点恢复（通过 append-only JSONL 实现）。
- **人类可读**：存储格式为纯文本 JSONL，便于调试和版本控制。

---

## 2. 存储布局

```
~/.config/shun-code/
├── sessions/
│   ├── 01HQ8J3K2M4N5P6Q7R8S9T0UV.jsonl   # 活跃会话
│   ├── 01HQ8J4L3M5N6P7Q8R9S0T1VW.jsonl   # 已归档
│   └── ...
├── projects.json                          # 会话索引（可选扩展）
└── config.toml                            # 全局配置（可选扩展）
```

- `sessions/` 目录下每个 `.jsonl` 文件对应一个 Session。
- 文件名为 `session_id`，按 ULID 生成，天然按时间排序。
- 使用 `directories` crate 解析平台相关的配置目录。

---

## 3. 核心数据模型

### 3.1 Session

```rust
pub struct Session {
    pub id: String,                 // ULID
    pub project_path: String,       // 当前工作目录
    pub created_at: u64,            // Unix timestamp (millis)
    pub updated_at: u64,
    pub model: String,              // 使用的模型名称
    pub status: SessionStatus,
    pub messages: Vec<Message>,
}

pub enum SessionStatus {
    Active,
    Idle,
    Archived,
}
```

### 3.2 Message

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Message {
    pub id: String,                 // ULID
    pub session_id: String,
    pub role: Role,
    pub created_at: u64,
    pub parts: Vec<Part>,
    pub token_count: Option<u64>,
    pub cost: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
    System,
    Developer,
}
```

### 3.3 Part

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Part {
    Text {
        text: String,
    },
    Image {
        source: ImageSource,
    },
    ToolUse {
        id: String,
        name: String,
        arguments: serde_json::Value,
    },
    ToolResult {
        tool_call_id: String,
        content: String,
        is_error: bool,
    },
    Reasoning {
        thinking: String,
        signature: Option<String>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ImageSource {
    Path { path: String },
    Base64 { media_type: String, data: String },
    Url { url: String },
}
```

---

## 4. JSONL 存储格式

每行一个独立 JSON 记录，append-only 写入：

```jsonl
// 文件头：Session 元数据
{"type":"session","id":"01HQ8J3K2M4N5P6Q7R8S9T0UV","project_path":"/home/user/project","created_at":1715779200000,"updated_at":1715779200000,"model":"claude-3-7-sonnet-20250219","status":"active"}

// Message 开始
{"type":"message_start","message_id":"01HQ8J4L3M5N6P7Q8R9S0T1VW","role":"user","created_at":1715779205000}

// Part 块（按 sequence 排序）
{"type":"part","message_id":"01HQ8J4L3M5N6P7Q8R9S0T1VW","sequence":0,"part":{"type":"text","text":"帮我分析这个错误"}}
{"type":"part","message_id":"01HQ8J4L3M5N6P7Q8R9S0T1VW","sequence":1,"part":{"type":"image","source":{"type":"base64","media_type":"image/png","data":"iVBORw0KGgo..."}}}

// Message 结束（统计信息）
{"type":"message_end","message_id":"01HQ8J4L3M5N6P7Q8R9S0T1VW","token_count":1250,"cost":0.0023}
```

**写入策略：**
- 运行时采用增量追加：每生成/接收到一个完整 `Message`，先写 `message_start`，再按序写每个 `part`，最后写 `message_end`。
- 初始创建 Session 时先写入 `session` 文件头。

**读取策略：**
- 按行 `BufReader` 流式读取。
- 遇到解析失败的行：打印警告并跳过，不中断后续恢复。
- 通过 `message_id` 将 `part` 聚合到对应 `Message`。

---

## 5. SessionManager API

```rust
pub struct SessionManager {
    sessions_dir: PathBuf,
    projects_index: PathBuf,
}

impl SessionManager {
    /// 基于当前工作目录和模型名称，创建新的 Active 会话
    pub fn create_session(&self, model: &str) -> Result<Session>;

    /// 列出所有会话元数据（按 updated_at 倒序）
    pub fn list_sessions(&self) -> Result<Vec<SessionMeta>>;
}

pub struct SessionMeta {
    pub id: String,
    pub project_path: String,
    pub created_at: u64,
    pub updated_at: u64,
    pub model: String,
    pub status: SessionStatus,
    pub message_count: usize,
}

    /// 从 JSONL 恢复完整 Session（含全部 Message 和 Part）
    pub fn load_session(&self, session_id: &str) -> Result<Session>;

    /// 全量覆写保存 Session（用于初始化或重建）
    pub fn save_session(&self, session: &Session) -> Result<()>;

    /// 运行时追加单条 Message（增量持久化）
    pub fn append_message(&self, session_id: &str, message: &Message) -> Result<()>;
}
```

- `SessionManager` 内部使用同步 `std::fs` I/O。
- 被 async `main.rs` 调用时，通过 `tokio::task::spawn_blocking` 包裹执行。
- `LoopState` 本身不持有持久化句柄；`main.rs` 在每次 `agent_loop` 返回后，统一调用 `SessionManager` 保存更新后的 `messages`。

---

## 6. 与现有代码的集成

### 6.1 类型迁移策略

采用**由内向外一次性迁移**：
1. 将 `agent/mod.rs` 中的 `Message` 和 `ContentBlock` 替换为新的 `Message` / `Part`。
2. 逐层修复 `provider/`（Anthropic / OpenAI 的 SSE 解析序列化）、`tools/`（ToolResult 格式）、`main.rs`（REPL 循环）。
3. 实现 `src/session/mod.rs` 的 `Session`、`SessionManager` 和 JSONL 读写。

### 6.2 具体改动点

| 文件 | 改动内容 |
|------|----------|
| `src/agent/mod.rs` | `Message` 增加 `id`、`session_id`、`role`、`created_at`、`parts`、`token_count`、`cost`；`ContentBlock` 重命名为 `Part`，字段对齐设计文档 |
| `src/provider/base_client.rs` | `stream_message` 签名中 `messages: &[Message]` 类型不变，但内部序列化逻辑适配新的 `Part` 结构 |
| `src/provider/client/anthropic_client.rs` | SSE 解析：`Text` → `Part::Text`；`Think` → `Part::Reasoning`；`ToolUse` → `Part::ToolUse` |
| `src/provider/client/openapi_client.rs` | 同上，适配 OpenAI 的 `tool_calls` / `reasoning_content` 到 `Part` |
| `src/tools/mod.rs` | `execute_tool_calls` 返回 `Vec<Part::ToolResult>`；不再返回裸 JSON |
| `src/main.rs` | 启动时调用 `SessionManager` 创建/恢复会话；每次 `agent_loop` 结束后保存 Session；提示符显示当前 session ID 前缀 |

### 6.3 运行时持久化流程

```
用户输入
    │
    ▼
┌─────────────────┐
│ 构造 User Message│ ──► SessionManager::append_message
│ (含 Text/Image) │
└─────────────────┘
    │
    ▼
agent_loop(state)
    │
    ├── 调用 provider 流式生成
    │       └── 实时聚合为 Assistant Message（Parts: Text/Reasoning/ToolUse）
    │
    ├── Assistant Message 写入 JSONL
    │       └── SessionManager::append_message
    │
    ├── 调用 tools 执行
    │       └── 生成 ToolResult Parts
    │
    └── User Message（ToolResult）写入 JSONL
            └── SessionManager::append_message
```

---

## 7. 错误处理策略

| 场景 | 处理策略 |
|------|----------|
| Session 目录创建失败 | `anyhow::bail!`，程序启动失败 |
| JSONL 写入失败 | 打印警告，降级为仅内存模式，不中断当前对话 |
| JSONL 某一行解析失败 | 打印警告并跳过该行，继续恢复后续记录 |
| `load_session` 找不到文件 | 返回 `anyhow::Error`，由调用方提示用户 |

---

## 8. 测试策略

- **单元测试（`src/session/`）**：
  - 在临时目录中创建 Session → 写入多条 Message → 读取恢复 → 断言结构一致。
  - 测试损坏 JSONL 行的跳过恢复行为。
- **单元测试（`src/agent/`）**：
  - 验证 `Message` / `Part` 的 `serde_json` 序列化与反序列化 round-trip。
- **集成测试（`src/main.rs` 流程模拟）**：
  - 通过 mock provider 跑一轮 agent_loop，验证持久化后的 JSONL 文件格式正确。

---

## 9. 新增依赖

```toml
[dependencies]
ulid = "1.1"           # Session / Message ID 生成
directories = "5.0"    # 平台相关的配置目录解析
```

时间戳使用 `std::time::SystemTime` 转换，不额外引入 `chrono`。

---

## 10. 关键设计决策

| 决策 | 说明 |
|------|------|
| JSONL 而非 SQLite | append-only 写入快，人类可读，便于调试 |
| ULID 而非 UUID | 时间可排序，文件名即创建顺序 |
| 每行独立 JSON | 支持流式恢复，损坏只丢一行 |
| 直接升级现有类型 | `agent::Message` / `ContentBlock` 直接替换为 `Message` / `Part`，避免长期维护两套模型 |
| ToolResult 放在 User Message | 符合 Anthropic / OpenAI API 的角色交替要求 |
| 同步 I/O + spawn_blocking | 文件 I/O 简单直接，避免在 session 层引入 async 复杂度 |
