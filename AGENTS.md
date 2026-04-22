# AGENTS.md —— learn-claude-code 项目指南

> 本文件面向 AI 编程助手。如果你刚刚拿到这个项目，请先阅读本文以了解代码结构、构建方式、开发约定和安全注意事项。

---

## 1. 项目概览

**fi-code** 是一个基于 Rust 构建的终端 AI Coding Agent CLI 程序。它通过 REPL 方式与用户交互，支持多轮对话、工具调用（文件读写、Bash 执行、网页抓取、代码搜索等）以及会话持久化。

- **语言**：Rust（Edition 2021）
- **包名**：`fi-code`
- **版本**：`0.1.0`
- **入口**：`src/main.rs`
- **运行时**：基于 `tokio` 的异步运行时

### 核心能力
1. **REPL 交互**：启动后让用户选择恢复历史会话或创建新会话，随后进入对话循环。
2. **模型对接**：统一封装了 OpenAI 兼容接口 与 Anthropic 接口，支持流式 SSE 响应解析。
3. **工具调用**：内置 6 个工具（`bash`、`read`、`write`、`edit`、`web_fetch`、`grep`），Agent 可根据模型返回的 `ToolUse` 自动执行并回传结果。
4. **会话持久化**：采用 JSONL（JSON Lines）格式将会话增量写入本地磁盘，支持中断后恢复。
5. **权限校验**：对 Bash 等高危操作进行风险分级（Allow / Ask / Deny），拦截 `sudo`、`rm -rf` 及常见注入攻击。
6. **配置管理**：支持通过 `~/.config/fi-code/config.json` 或 `config.jsonc` 管理模型和 Provider 设置，支持 JSONC 注释、环境变量占位符（`{env:VAR_NAME}`）以及文件系统事件热重载（500ms 防抖）。

---

## 2. 技术栈与关键依赖

| 依赖 | 用途 |
|------|------|
| `tokio` | 异步运行时 |
| `reqwest` | HTTP 客户端，支持 SSE 流式请求 |
| `serde` / `serde_json` | 序列化与反序列化 |
| `anyhow` | 简化错误传播 |
| `rustyline` | 终端行读取与历史记录 |
| `colored` | 终端彩色输出 |
| `dotenvy` | 加载 `.env` 环境变量 |
| `ulid` | 生成 Session / Message ID |
| `directories` | 解析平台相关的配置目录 |
| `notify` | 配置文件热重载的文件系统事件监听 |
| `jsonc-parser` | 解析带注释的 JSONC 配置文件 |
| `html2md` | 网页 HTML 转 Markdown |
| `regex` | `grep` 工具的正则匹配 |
| `async-trait` | 异步 trait 支持 |
| `futures` / `bytes` / `rand` | 流处理、字节操作、随机数 |

**开发依赖**：`wiremock`（HTTP Mock）、`tempfile`（临时目录测试）。

---

## 3. 代码组织与模块划分

```
src/
├── main.rs                 # 程序入口：REPL、会话选择、持久化调度
├── agent/
│   └── mod.rs              # Agent 核心循环：LoopState、run_one_turn、agent_loop
├── provider/
│   ├── mod.rs              # 模块聚合与重新导出
│   ├── provider.rs         # Model / Provider：环境变量读取、客户端工厂
│   ├── base_client.rs      # AIClient trait、FinishReason、RetryConfig、send_with_retry
│   └── client/
│       ├── mod.rs
│       ├── anthropic_client.rs   # Anthropic SSE 流式客户端
│       └── openapi_client.rs     # OpenAI 兼容 SSE 流式客户端
├── session/
│   ├── mod.rs              # 子模块声明与公共导出
│   ├── message.rs          # Message、Part、Role、ImageSource、MessageBuilder
│   └── session.rs          # SessionManager、Session、SessionMeta、JSONL 读写
├── tools/
│   ├── mod.rs              # 工具注册表初始化、tool_schema、tool_call、execute_tool_calls
│   ├── basic_tools.rs      # BasicTool：bash/read/write/edit/web_fetch/grep 的底层实现
│   ├── tools_registry.rs   # ToolsRegistry：工具的注册与查找
│   └── tools_type.rs       # ToolHandler trait、ToolParameter、ToolParams
├── config/
│   ├── mod.rs              # 模块入口，导出 Config 和相关类型
│   ├── config.rs           # Config 加载、JSONC 解析、环境变量占位符、热重载监听
│   └── models.rs           # 配置数据模型：Config / ProviderConfig / ModelConfig / ModelLimits
├── permission/
│   ├── mod.rs
│   └── permission.rs       # 权限风险分级与交互式确认
```

> **注意**：`src/commands`、`src/coroutine`、`src/plugins`、`src/skills`、`src/task`、`src/teammate` 目前为空目录或仅含框架代码，属于预留或待扩展模块。
>
> `src/mcp/` 已实现 MCP（Model Context Protocol）支持，包含协议类型（`types.rs`）、客户端 trait（`client.rs`）、stdio/HTTP 传输层（`transport.rs`）和多服务器管理器（`manager.rs`）。

---

## 4. 构建与运行

### 4.1 配置方式

支持两种配置方式，**优先级：环境变量 > 配置文件 > 错误提示**。

#### 方式一：环境变量（最高优先级）

运行前设置对应 Provider 的环境变量（支持 `.env` 文件）：

**OpenAI 兼容：**
```bash
OPENAI_API_KEY=sk-...
OPENAI_BASE_URL=https://api.openai.com/v1
OPENAI_MODEL_NAME=gpt-4o
```

**Anthropic：**
```bash
ANTHROPIC_API_KEY=sk-ant-...
# 或 ANTHROPIC_AUTH_TOKEN
ANTHROPIC_BASE_URL=https://api.anthropic.com
ANTHROPIC_MODEL_NAME=claude-3-7-sonnet-20250219
```

#### 方式二：配置文件（环境变量不存在时自动降级）

配置文件路径（按优先级查找）：
1. `~/.config/fi-code/config.jsonc`
2. `~/.config/fi-code/config.json`

**配置文件格式示例：**
```json
{
  "model": "my-model",
  "provider": {
    "openai": {
      "npm": "@ai-sdk/openai-compatible",
      "name": "My Provider",
      "options": {
        "apiKey": "{env:MY_API_KEY}",
        "baseURL": "https://api.example.com/v1",
        "timeout": 300000,
        "chunkTimeout": 10000
      },
      "models": {
        "my-model": {
          "name": "My Model",
          "limit": { "context": 200000, "output": 65536 }
        }
      }
    }
  }
}
```

**特性说明：**
- `config.jsonc` 支持 `//` 和 `/* */` 注释
- `apiKey` 支持 `{env:VAR_NAME}` 占位符语法，启动时自动替换为对应环境变量值
- 配置文件变更后自动热重载（500ms 防抖）

### 4.2 常用命令

```bash
# 编译
cargo build

# 运行
cargo run

# 运行测试
cargo test

# 格式化代码
cargo fmt

# Clippy 静态检查
cargo clippy
```

### 4.3 运行时的数据存储

会话数据以 `.jsonl` 格式保存在平台配置目录下：
- Linux: `~/.config/fi-code/sessions/`
- macOS: `~/Library/Application Support/fi-code/sessions/`
- Windows: `%APPDATA%\fi-code\sessions\`

配置文件保存在同一配置目录下：
- Linux: `~/.config/fi-code/config.json` 或 `config.jsonc`
- macOS: `~/Library/Application Support/fi-code/config.json` 或 `config.jsonc`
- Windows: `%APPDATA%\fi-code\config.json` 或 `config.jsonc`

---

## 5. 测试策略与现状

- **单元测试**：各模块的 `#[cfg(test)]` 内嵌测试，覆盖工具调用、权限校验、Session 创建/加载/追加、损坏 JSONL 容错等场景。
- **当前状态**：共 26 个测试，全部通过。
- **运行方式**：直接执行 `cargo test` 即可。
- **关键测试文件**：
  - `src/tools/mod.rs`：注册表功能、各工具的 `tool_call` 调用。
  - `src/tools/basic_tools.rs`：底层读写、Bash、Grep 的独立测试。
  - `src/session/session.rs`：SessionManager 的创建、追加、加载、损坏行跳过。
  - `src/permission/permission.rs`：Allow / Ask / Deny 的风险分级规则校验。
  - `src/config/mod.rs`：Config 默认构造、JSON/JSONC 解析、环境变量占位符替换。

---

## 6. 代码风格与开发约定

1. **注释风格**：项目内大量使用中文注释，并夹杂对 Rust 基础概念（`mod`、`use`、`trait`、`LazyLock`、`Box<dyn>` 等）的教学式解释。修改代码时建议保持中文注释风格。
2. **错误处理**：统一使用 `anyhow::Result` 做错误传播；模块内部对外暴露 `Result<T>` 时优先使用 `anyhow`。
3. **同步 I/O 在异步上下文中的使用**：`SessionManager` 内部使用同步 `std::fs` I/O，在 `main.rs` 的 `tokio::main` 中通过 `tokio::task::spawn_blocking` 包裹调用，避免阻塞异步事件循环。
4. **模块导出**：每个模块的 `mod.rs` 负责声明子模块并重新导出（`pub use`）常用类型，减少外部调用时的路径层级。
5. **全局单例**：工具注册表使用 `std::sync::LazyLock` 实现懒加载单例，并通过 `init_tools()` 显式触发初始化。
6. **Schema 生成**：工具的 JSON Schema 完全由 `ToolsRegistry` 动态生成，新增工具时只需在 `src/tools/mod.rs` 的 `REGISTRY` 初始化闭包中注册，无需手动维护 schema 代码。

---

## 7. 安全注意事项

在修改或扩展以下逻辑时，必须保持现有的安全防线：

### 7.1 路径逃逸防护
`BasicTool::safe_path` 使用 `canonicalize` 与 `starts_with` 检查，确保所有文件操作不会超出程序启动时的工作目录。新增文件相关工具时必须复用此检查。

### 7.2 Bash 执行沙箱
`BasicTool::run_bash` 做了以下安全处理：
- 清除所有继承的环境变量（`env_clear()`），阻断 `LD_PRELOAD`、`BASH_ENV` 等注入通道。
- 仅保留最小必要环境变量 `PATH=/usr/bin:/bin` 与 `HOME`。
- 通过 `mpsc::channel` + `recv_timeout(Duration::from_secs(120))` 实现 120 秒超时。

### 7.3 权限分级系统
`permission/permission.rs` 对工具调用进行风险判定：
- **Deny（直接拒绝）**：`bash` 命令中包含 `sudo`、`rm -rf` / `rm -fr`、常见注入字符（`;`、`|`、`&&`、`||`、`` ` ``、`$(`、`>`、`<`、`&`）。
- **Allow（直接放行）**：`read`、`read_file`、`grep` 等只读操作。
- **Ask（交互确认）**：其他工具（如 `write`、`edit`、`web_fetch`）以及安全的 `bash` 命令会提示用户输入 `Yes/No`。

### 7.4 输出截断
所有工具返回内容均做了 `chars().take(50000)` 截断，防止超大输出一次性撑爆 LLM 上下文。

---

## 8. 设计文档参考

- `docs/session-desgin.md`：详细的 Session-Message-Part 持久化系统设计，包含 JSONL 格式规范、存储布局、`SessionManager` API、错误处理策略与测试策略。

---

## 9. 给 AI 助手的快速 Checklist

- [ ] 修改代码后运行 `cargo test` 确保全部通过。
- [ ] 新增工具时记得在 `src/tools/mod.rs` 的 `REGISTRY` 中注册，并补充单元测试。
- [ ] 涉及文件系统或 Bash 的操作必须复用现有的 `safe_path` / 权限检查 / 超时 / 输出截断机制。
- [ ] 保持中文注释风格，对新增的 Rust 语法或设计模式可适当补充说明。
- [ ] `Cargo.toml` 或环境变量相关变更需在本文对应章节同步更新。
- [ ] 新增或修改配置模块功能时，同步更新本文第 4.1 节（配置方式）的示例和说明。
- [ ] 配置文件格式变更时，同步更新设计文档 `docs/superpowers/specs/` 中的相关 spec。
