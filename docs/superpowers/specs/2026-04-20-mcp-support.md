# MCP 支持设计文档

## 背景与目标

为 `shun-code` 新增 MCP（Model Context Protocol）支持，使其能够连接本地（stdio）和远程（HTTP）MCP 服务器，并将 MCP 工具与现有 basic_tools 一起注入系统提示词。通过 Two-Step-Discovery 机制优化 Token 使用：系统提示词中只展示工具名称和描述，当 LLM 决策调用时再获取完整 schema 并重新决策。

## 文件结构

```
src/
├── mcp/
│   ├── mod.rs              # 模块入口，导出 McpManager / McpClient
│   ├── client.rs           # McpClient trait
│   ├── transport.rs        # LocalClient (stdio) + RemoteClient (HTTP POST)
│   ├── types.rs            # JSON-RPC + MCP 协议类型
│   └── manager.rs          # McpManager：多服务器管理、自动重连、状态监控
├── config/
│   └── models.rs           # 扩展：新增 McpServerConfig / McpServerType
├── tools/
│   ├── mod.rs              # 修改：MCP 路由 + two-step 调用
│   └── tools_registry.rs   # 修改：混合 schema 生成（basic 完整 + MCP 轻量）
└── agent/
    └── mod.rs              # 修改：ToolUse 处理插入 MCP Two-Step 分支
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
pub struct InitializeParams { ... }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeResult { ... }

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

### LocalClient — stdio（`src/mcp/transport.rs`）

- 通过 `tokio::process::Command` 启动子进程
- 通过 `stdin` 发送 JSON-RPC（每行一个 JSON + `\n`）
- 通过 `stdout` 读取响应行
- `Drop` 时 `kill()` 子进程

### RemoteClient — HTTP POST（`src/mcp/transport.rs`）

- `reqwest::Client` 发送 JSON-RPC POST 请求
- 默认 30s 超时（`timeout(Duration::from_secs(30))`）
- 支持自定义 headers（如 `Authorization`）

## McpManager

### 多服务器管理

```rust
pub struct McpManager {
    clients: Arc<RwLock<HashMap<String, Box<dyn McpClient>>>>,
    configs: HashMap<String, McpServerConfig>,
    status: Arc<RwLock<HashMap<String, McpServerStatus>>>,
    tools_summary: HashMap<String, String>,     // full_name → description
    tools_full: HashMap<String, McpTool>,       // full_name → McpTool
    max_retries: u32,
}
```

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

### ToolsRegistry 扩展（`src/tools/tools_registry.rs`）

```rust
pub struct ToolsRegistry {
    pub entries: HashMap<String, RegistryEntry>,
    pub mcp_manager: Option<Arc<McpManager>>,
}
```

### 混合 Schema 生成

```rust
pub fn tool_schema(&self) -> Vec<serde_json::Value> {
    let mut schemas = Vec::new();
    
    // basic_tools：完整 schema（含 parameters）
    for (_, entry) in &self.entries {
        schemas.push(entry.schema.clone());
    }
    
    // mcp_tools：轻量 schema（仅 name + description，无 parameters）
    if let Some(mcp) = &self.mcp_manager {
        for (full_name, desc) in mcp.tools_list() {
            schemas.push(serde_json::json!({
                "type": "function",
                "function": {
                    "name": full_name,
                    "description": desc,
                }
            }));
        }
    }
    
    schemas
}
```

### 路由分发

```rust
pub async fn tool_call(&self, name: &str, params: ToolParams) -> Result<String, String> {
    if name.starts_with("mcp:") {
        // MCP 工具
        let args = extract_json_params(params)?;
        let mcp = self.mcp_manager.as_ref().ok_or("MCP 未初始化")?;
        let result = mcp.tool_call(name, args).await
            .map_err(|e| format!("MCP 调用失败: {}", e))?;
        Ok(extract_text_from_result(result))
    } else {
        // 本地工具
        let entry = self.entries.get(name).ok_or("未知工具")?;
        entry.handler.call(name, params)
    }
}
```

## Agent Loop — Two-Step-Discovery

### 流程

```
Step 1 — System Prompt（轻量）
  basic_tools: 完整 schema（name, description, parameters）
  mcp_tools:  轻量信息（name, description）— 无 parameters

  LLM 看到所有工具，决定调用 mcp:shadcn/read_component

Step 2 — Schema 补充（LLM 重新决策）
  Agent 拦截 ToolUse：
    a. 识别到 mcp: 前缀
    b. 调用 mcp_manager.tool_schema("mcp:shadcn/read_component")
    c. 构造补充消息：完整 input_schema + "请提供正确参数"
    d. 追加到消息历史，重新调用 LLM
    e. LLM 返回带正确参数的 ToolUse

Step 3 — 执行调用
  调用 tools::tool_call() → McpManager::tool_call()
  结果回传，继续 agent_loop
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

## 生命周期管理

- **LocalClient Drop**：`impl Drop for LocalClient { fn drop(&mut self) { let _ = self.process.start_kill(); } }`
- **McpManager**：程序退出时自动 Drop 所有 client，子进程随之终止
- **RemoteClient**：无状态，无需特殊清理

## 测试策略

| 测试目标 | 方式 |
|----------|------|
| Config 模型扩展（含 mcp 字段） | 单元测试：验证 JSON 解析 |
| JSON-RPC 序列化/反序列化 | 单元测试：验证 Request/Response 格式 |
| LocalClient stdio 通信 | Mock 子进程（shell 脚本模拟 MCP 服务器） |
| RemoteClient HTTP 通信 | `wiremock` 模拟远程端点 |
| McpManager 多服务器聚合 | 集成测试：多个 mock 服务器，验证工具合并 |
| McpManager 自动重连 | Mock client 返回错误，验证退避重试逻辑 |
| 混合 schema 生成 | 验证 basic_tools 完整 + mcp_tools 轻量 |
| MCP 路由 | 验证 `mcp:` 前缀正确路由，普通名称路由到本地 |
