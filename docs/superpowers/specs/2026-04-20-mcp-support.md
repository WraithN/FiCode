# MCP 支持设计文档

## 背景与目标

为 `shun-code` 新增 MCP（Model Context Protocol）支持，使其能够连接本地（stdio）和远程（HTTP）MCP 服务器，并将 MCP 工具与现有 basic_tools 一起注入系统提示词。通过 Two-Step-Discovery 机制优化 Token 使用：系统提示词中只展示工具名称和描述，当 LLM 决策调用时再获取完整 schema 并重新决策。

## 文件结构

```
src/
├── mcp/
│   ├── mod.rs              # 模块入口，导出 McpManager / McpClient / types
│   ├── client.rs           # McpClient trait
│   ├── transport.rs        # LocalClient (stdio) + RemoteClient (HTTP POST)
│   ├── types.rs            # JSON-RPC + MCP 协议类型
│   └── manager.rs          # McpManager：多服务器管理、自动重连、状态监控
├── config/
│   └── models.rs           # 扩展：新增 McpServerConfig / McpServerType
├── tools/
│   ├── mod.rs              # 修改：MCP 路由 + two-step 调用 + 混合 schema
│   └── tools_registry.rs   # 保持现有结构不变（不直接耦合 MCP）
└── agent/
    └── mod.rs              # 保持现有结构不变
    └── agent.rs            # 修改：ToolUse 处理插入 MCP Two-Step 分支
```

## 数据模型

### Config 扩展（`src/config/models.rs`）

```rust
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct Config {
    pub model: String,
    pub provider: HashMap<String, ProviderConfig>,
    pub mcp: Option<HashMap<String, McpServerConfig>>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct McpServerConfig {
    #[serde(rename = "type")]
    pub server_type: McpServerType,
    pub enabled: bool,
    pub command: Option<Vec<String>>,      // local only
    pub url: Option<String>,               // remote only
    pub headers: Option<HashMap<String, String>>, // remote only
}

// 必须手动实现 Default，因为 Config derive 了 Default
impl Default for McpServerConfig {
    fn default() -> Self {
        Self {
            server_type: McpServerType::Local,
            enabled: true,
            command: None,
            url: None,
            headers: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum McpServerType {
    Local,
    Remote,
}
```

### MCP 协议类型（`src/mcp/types.rs`）

基于 JSON-RPC 2.0：

```rust
pub const MCP_PROTOCOL_VERSION: &str = "2024-11-05";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest<T> {
    pub jsonrpc: String,
    pub id: u64,
    pub method: String,
    pub params: T,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse<T> {
    pub jsonrpc: String,
    pub id: u64,
    #[serde(flatten)]
    pub result: JsonRpcResult<T>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum JsonRpcResult<T> {
    Success { result: T },
    Error { error: JsonRpcError },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
}

// Initialize
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeParams {
    pub protocol_version: String,
    pub capabilities: ClientCapabilities,
    pub client_info: ClientInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClientCapabilities {
    pub tools: Option<ToolsCapability>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsCapability {
    pub list_changed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeResult {
    pub protocol_version: String,
    pub capabilities: ServerCapabilities,
    pub server_info: ServerInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerCapabilities {
    pub tools: Option<ToolsCapability>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

// Tools
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListToolsResult {
    pub tools: Vec<McpTool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallToolParams {
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallToolResult {
    pub content: Vec<ToolContent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: String,
}
```

## McpClient Trait 与 Transport

### McpClient Trait（`src/mcp/client.rs`）

```rust
#[async_trait]
pub trait McpClient: Send + Sync {
    async fn initialize(&mut self) -> Result<InitializeResult>;
    async fn list_tools(&self) -> Result<ListToolsResult>;
    async fn call_tool(&self, name: &str, arguments: serde_json::Value) -> Result<CallToolResult>;
}
```

**注意**：`initialize` 使用 `&mut self`，因为 stdio 客户端需要可变引用写入 stdin；`list_tools` 和 `call_tool` 使用 `&self`，通过内部可变性（`tokio::sync::Mutex`）实现。

### LocalClient — stdio（`src/mcp/transport.rs`）

- 通过 `tokio::process::Command` 启动子进程
- 通过 `stdin` 发送 JSON-RPC（每行一个 JSON + `\n`）
- 通过 `stdout` 读取响应行
- `stdin` 和 `stdout` 用 `tokio::sync::Mutex` 包裹，使 `list_tools`/`call_tool` 可用 `&self`
- `Drop` 时 `kill()` 子进程

```rust
pub struct LocalClient {
    process: Child,
    stdin: tokio::sync::Mutex<tokio::process::ChildStdin>,
    stdout: tokio::sync::Mutex<BufReader<tokio::process::ChildStdout>>,
    request_id: AtomicU64,
}
```

### RemoteClient — HTTP POST（`src/mcp/transport.rs`）

- `reqwest::Client` 发送 JSON-RPC POST 请求
- 默认 30s 超时（`timeout(Duration::from_secs(30))`）
- 支持自定义 headers（如 `Authorization`）
- `reqwest::Client` 和 `AtomicU64` 天然支持 `&self` 操作，无需额外锁

## McpManager

### 多服务器管理

```rust
pub struct McpManager {
    clients: Arc<RwLock<HashMap<String, Box<dyn McpClient>>>>,
    configs: HashMap<String, McpServerConfig>,
    status: Arc<RwLock<HashMap<String, McpServerStatus>>>,
    tools_summary: Arc<RwLock<HashMap<String, String>>>,     // full_name → description
    tools_full: Arc<RwLock<HashMap<String, McpTool>>>,       // full_name → McpTool
    max_retries: u32,
}
```

**关键修正**：`tools_summary` 和 `tools_full` 使用 `Arc<RwLock<...>>` 包裹，因为 `connect_and_load_tools` 和 `reconnect` 需要从 `&self` 方法中修改它们，且 `McpManager` 会被 `Arc` 共享。

### 初始化流程

1. 遍历 `config.mcp` 中所有 `enabled = true` 的服务器
2. 根据 `type` 创建 `LocalClient` 或 `RemoteClient`
3. 发送 `initialize` 握手
4. 发送 `tools/list` 获取工具列表
5. 提取 `{name, description}` 存入 `tools_summary`
6. 完整 `McpTool` 存入 `tools_full`
7. 单个服务器失败不影响其他（部分成功策略）

### 命名空间

- 格式：`mcp:{server_name}/{tool_name}`
- 示例：`mcp:shadcn/read_component`

### 自动重连

- **触发条件**：`tool_call` 返回错误
- **策略**：指数退避（1s, 2s, 4s），最大 3 次
- **懒重连**：不在后台轮询，只在调用时触发
- **状态流转**：`Healthy` → `Reconnecting` → `Healthy` 或 `Failed`
- **注意**：`reconnect` 只重建 client 和重新 `initialize`，**不重复获取工具列表**（工具列表已在初始化时缓存）

### 状态监控

```rust
pub enum McpServerStatus {
    Healthy,
    Reconnecting,
    Failed(String),
}

pub fn server_status(&self, name: &str) -> Option<McpServerStatus>;
pub fn all_status(&self) -> Vec<(String, McpServerStatus)>;
```

## 工具注册表集成

### 现有结构约束

现有 `ToolsRegistry`（`src/tools/tools_registry.rs`）的结构为：

```rust
pub struct ToolsRegistry {
    tools_map: HashMap<Arc<str>, ToolSlot>,
}
```

它是一个 `LazyLock` 全局单例，在程序启动时立即初始化。**不直接修改 `ToolsRegistry` 结构**，而是通过 `src/tools/mod.rs` 中的全局函数来合并 MCP 工具。

### MCP 全局状态（`src/tools/mod.rs`）

```rust
use std::sync::{Arc, RwLock};

static MCP_MANAGER: RwLock<Option<Arc<McpManager>>> = RwLock::new(None);

pub fn set_mcp_manager(manager: Arc<McpManager>) {
    let mut lock = MCP_MANAGER.write().unwrap();
    *lock = Some(manager);
}

pub fn get_mcp_manager() -> Option<Arc<McpManager>> {
    MCP_MANAGER.read().unwrap().clone()
}
```

### 混合 Schema 生成（`src/tools/mod.rs`）

现有 `tool_schema()` 返回内部格式 `[{"name": ..., "description": ..., "input_schema": ...}]`。
Anthropic 客户端直接使用该格式；OpenAI 客户端通过 `convert_tools_schema` 转换为 `{"type": "function", "function": {...}}`。

```rust
pub fn tool_schema() -> serde_json::Value {
    let mut schemas = Vec::new();
    
    // basic_tools：完整 schema（从注册表获取）
    let basic = REGISTRY.tool_schema();
    if let Some(arr) = basic.as_array() {
        schemas.extend(arr.iter().cloned());
    }
    
    // mcp_tools：轻量 schema（仅 name + description，input_schema 为空对象）
    if let Ok(lock) = MCP_MANAGER.read() {
        if let Some(mcp) = lock.as_ref() {
            for (full_name, desc) in mcp.tools_list() {
                schemas.push(serde_json::json!({
                    "name": full_name,
                    "description": desc,
                    "input_schema": serde_json::Value::Object(serde_json::Map::new()),
                }));
            }
        }
    }
    
    serde_json::Value::Array(schemas)
}
```

**说明**：`input_schema` 设为空对象 `{}`，OpenAI 转换后会得到 `parameters: {}`，Anthropic 直接传递 `{}`。这比完整 schema 大幅节省 token，同时保留工具可调用性。

### 路由分发（`src/tools/mod.rs`）

现有 `tool_call` 和 `execute_tool_calls` 是**同步**的。MCP 调用需要**异步**，因此需要升级为 `async`：

```rust
pub async fn tool_call(
    name: &str,
    input: &HashMap<String, serde_json::Value>,
) -> Result<String, String> {
    if name.starts_with("mcp:") {
        let input_json = serde_json::to_value(input).unwrap_or_default();
        let mcp = get_mcp_manager().ok_or("MCP manager not initialized".to_string())?;
        match mcp.tool_call(name, input_json).await {
            Ok(result) => {
                let texts: Vec<String> = result.content.iter().map(|c| c.text.clone()).collect();
                Ok(texts.join("\n"))
            }
            Err(e) => Err(format!("MCP call failed: {}", e)),
        }
    } else {
        let input_json = serde_json::to_value(input).unwrap_or_default();
        let params = vec![ToolParameter::Json(input_json)];
        REGISTRY.call(name, params)
    }
}

pub async fn execute_tool_calls(parts: &[Part]) -> Vec<Part> {
    // ... 原有逻辑，但 tool_call(name, &input).await 改为 await
}
```

**调用链变更**：
- `agent.rs`：`execute_tool_calls(&content_blocks)` → `execute_tool_calls(&content_blocks).await`
- `provider/mod.rs` 中 `pub use crate::tools::{execute_tool_calls, tool_call};` 保持，但调用方需注意 `async`

## Agent Loop — Two-Step-Discovery

### 流程

```
Step 1 — System Prompt（轻量）
  basic_tools: 完整 schema（name, description, input_schema）
  mcp_tools:  轻量信息（name, description, input_schema: {}）

  LLM 看到所有工具，决定调用 mcp:shadcn/read_component

Step 2 — Schema 补充（LLM 重新决策）
  Agent 拦截 ToolUse：
    a. 识别到 mcp: 前缀
    b. 调用 mcp_manager.tool_schema("mcp:shadcn/read_component")
    c. 构造补充消息：完整 input_schema + "请提供正确参数"
    d. 追加到消息历史，重新调用 LLM
    e. LLM 返回带正确参数的 ToolUse

Step 3 — 执行调用
  调用 tools::tool_call().await → McpManager::tool_call()
  结果回传，继续 agent_loop
```

### 插入位置（`src/agent/agent.rs`）

在 `run_one_turn` 中，Assistant 消息已追加到 `state.messages`，且 `finish_reason == ToolUse` 时：

1. 先检查所有 `Part::ToolUse` 中是否有 `mcp:` 前缀
2. 如果有，对于每个 MCP 工具：
   - 获取完整 `input_schema`
   - 构造补充提示消息
   - 追加到 `state.messages`
3. 重新调用 `client.stream_message`（或复用 `run_one_turn` 递归）
4. 等待 LLM 返回带参数的 ToolUse
5. 然后执行所有工具调用（包括 basic 和 MCP）

**简化实现**：在检测到 MCP ToolUse 后，直接修改 `content_blocks`，移除无参数的 MCP ToolUse，追加 schema 补充消息，再触发一次 LLM 调用。得到正确参数后，再统一进入 `execute_tool_calls`。

具体伪代码：

```rust
// 在 run_one_turn 中，检测到 finish_reason == ToolUse 后
let mcp_tools_without_args: Vec<&Part> = content_blocks.iter()
    .filter(|p| matches!(p, Part::ToolUse { name, .. } if name.starts_with("mcp:")))
    .collect();

if !mcp_tools_without_args.is_empty() {
    // 构造 schema 补充消息
    let mut schema_texts = Vec::new();
    for tool in mcp_tools_without_args {
        if let Part::ToolUse { name, .. } = tool {
            if let Some(mcp) = tools::get_mcp_manager() {
                if let Some(schema) = mcp.tool_schema(name) {
                    schema_texts.push(format!(
                        "工具 `{}` 的完整参数格式：\n```json\n{}\n```",
                        name,
                        serde_json::to_string_pretty(&schema.input_schema).unwrap_or_default()
                    ));
                }
            }
        }
    }
    
    // 追加到消息历史
    state.messages.push(Message::new(
        session_id.clone(),
        Role::User,
        vec![Part::Text {
            text: format!("请为以下 MCP 工具提供正确的参数：\n{}", schema_texts.join("\n\n")),
        }],
    ));
    
    // 重新运行一轮获取参数
    // 这里需要递归调用 run_one_turn，或手动重复流式调用逻辑
    // ...（详细实现根据 agent.rs 当前结构确定）
}
```

## 错误处理

| 场景 | 处理 |
|------|------|
| MCP 服务器连接失败 | 警告日志，跳过该服务器，继续初始化其他 |
| MCP 初始化握手失败 | 同上 |
| `list_tools` 超时（30s） | 该服务器工具不加入注册表 |
| `call_tool` 超时（30s） | 返回错误，触发自动重连，重试一次 |
| 子进程崩溃 | `tool_call` 失败 → 重连 → 重试 |
| 工具名格式错误 | `anyhow!` 明确报错 |
| 重连耗尽（3 次） | 状态设为 `Failed`，后续调用直接返回错误 |
| MCP Manager 未初始化但调用 mcp: 工具 | 返回错误 "MCP manager not initialized" |

## 生命周期管理

- **LocalClient Drop**：`impl Drop for LocalClient { fn drop(&mut self) { let _ = self.process.start_kill(); } }`
- **McpManager**：程序退出时自动 Drop 所有 client，子进程随之终止
- **RemoteClient**：无状态，无需特殊清理
- **MCP_MANAGER 全局状态**：程序启动时由 `main.rs` 设置，运行期间只读

## 测试策略

| 测试目标 | 方式 |
|----------|------|
| Config 模型扩展（含 mcp 字段） | 单元测试：验证 JSON 解析，包括 Default 行为 |
| JSON-RPC 序列化/反序列化 | 单元测试：验证 Request/Response 格式 |
| LocalClient stdio 通信 | Mock 子进程（shell 脚本模拟 MCP 服务器） |
| RemoteClient HTTP 通信 | `wiremock` 模拟远程端点 |
| McpManager 多服务器聚合 | 集成测试：多个 mock 服务器，验证工具合并 |
| McpManager 自动重连 | Mock client 返回错误，验证退避重试逻辑 |
| 混合 schema 生成 | 验证 basic_tools 完整 + mcp_tools 轻量 |
| MCP 路由 | 验证 `mcp:` 前缀正确路由，普通名称路由到本地 |
| Config Default | 验证 `Config::default()` 不因 `McpServerConfig` 而 panic |
