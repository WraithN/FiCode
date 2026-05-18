<!-- From: /home/nan/fi-code/AGENTS.md -->
# AGENTS.md —— fi-code 项目指南

> 本文件面向 AI 编程助手。如果你刚刚拿到这个项目，请先阅读本文以了解代码结构、构建方式、开发约定和安全注意事项。
> 本文档基于项目实际内容编写，所有信息均来自代码库中的配置文件与源代码。

---

## 1. 项目概览

**fi-code** 是一个基于 Rust 构建的终端 AI Coding Agent。它通过多模式交互（REPL、TUI、HTTP Server、Desktop）与用户协作，支持多轮对话、工具调用（文件读写、Bash 执行、网页抓取、代码搜索、Git 操作等）、任务拆分、会话持久化以及 MCP（Model Context Protocol）扩展。

- **语言**：Rust（Edition 2021）
- **版本**：`0.1.0`
- **包结构**：Cargo Workspace，核心逻辑在 `fi-code-core`，多个前端入口（CLI / TUI / Server / Desktop）
- **运行时**：基于 `tokio` 的异步运行时
- **代码规模**：`crates/core/src/` 约 61 个 `.rs` 文件，总计约 10,700+ 行代码；整个 Workspace 约 95 个 `.rs` 文件

### 核心能力

1. **多模式交互**：
   - **CLI REPL**：传统命令行交互模式（`fi-code-cli -i`）
   - **TUI**：基于 `ratatui` 的全终端界面（`fi-code-tui`）
   - **HTTP Server**：REST API + SSE 流式响应（`fi-code-server` 或 `fi-code-cli server`）
   - **Desktop**：基于 Tauri v2 的桌面应用（`fi-code-desktop`），采用"Tauri 壳 + 嵌入式 Sidecar"架构

2. **模型对接**：统一封装了 OpenAI 兼容接口与 Anthropic 接口，支持流式 SSE 响应解析。内置重试机制（指数退避 + Full Jitter）。

3. **工具调用**：内置 **20 个本地工具**，Agent 可根据模型返回的 `ToolUse` 自动执行并回传结果：
   - 基础工具：`bash`、`read`、`write`、`edit`、`web_fetch`、`grep`、`glob`
   - Git 工具：`git`、`git_status`、`git_diff`、`git_add`、`git_commit`、`git_log`、`git_worktree`
   - 任务工具：`create_task_plan`、`handle_task_plan`
   - 交互工具：`ask_for_question`
   - Skill 工具：`use_skill`
   - MCP 工具：动态加载，以 `mcp:` 为前缀

4. **会话持久化**：采用 JSONL（JSON Lines）格式将会话增量写入本地磁盘，支持中断后恢复。格式为：`session` 头 → `message_start` → `part` 行（xN）→ `message_end`。

5. **权限校验**：对 Bash 等高危操作进行风险分级（Allow / Ask / Deny），拦截 `sudo`、`rm -rf` 及常见注入攻击。

6. **配置管理**：支持通过 `~/.config/fi-code/config.json` 或 `config.jsonc` 管理模型和 Provider 设置，支持 JSONC 注释、环境变量占位符（`{env:VAR_NAME}`）以及文件系统事件热重载（500ms 防抖）。

7. **MCP 支持**：完整实现 Model Context Protocol，支持多服务器管理（stdio / HTTP 传输），自动重连（最多 3 次，指数退避）。

8. **Skills 系统**：可扩展的 Skill 注册与加载机制，Agent 可通过 `use_skill` 工具按需加载项目内的 Skill 指令。

---

## 2. 技术栈与关键依赖

### Rust 依赖（Workspace 级别）

根 `Cargo.toml` 中 `[workspace.dependencies]` 声明的共享依赖：

| 依赖 | 版本 | 用途 |
|------|------|------|
| `tokio` | `1.35` (full) | 异步运行时 |
| `reqwest` | `0.13.2` (json, blocking, stream) | HTTP 客户端，支持 SSE 流式请求 |
| `serde` / `serde_json` | `1.0` | 序列化与反序列化 |
| `anyhow` | `1.0` | 简化错误传播 |
| `axum` | `0.7` | HTTP Server 框架 |
| `tower-http` | `0.5` (cors) | HTTP CORS 中间件 |
| `tokio-stream` | `0.1` (sync) | 流处理辅助 |
| `async-trait` | `0.1.89` | 异步 trait 支持 |
| `futures` | `0.3` | 异步流组合 |

### Crate 级独有依赖

| Crate | 关键依赖 | 用途 |
|-------|----------|------|
| `fi-code-core` | `html2md`, `regex`, `serde_yaml`, `dotenvy`, `ulid`, `directories`, `dirs`, `notify`, `jsonc-parser`, `walkdir`, `unicode-width`, `once_cell`, `glob`, `similar`, `chrono` | 核心逻辑：网页抓取、正则、配置热重载、JSONC 解析、目录遍历、diff 计算等 |
| `fi-code-core` (dev) | `wiremock`, `tempfile`, `insta` | HTTP Mock、临时目录、快照测试 |
| `fi-code-cli` | `clap`, `colored` | 命令行参数解析、彩色输出 |
| `fi-code-tui` | `ratatui` (unstable-rendered-line-info), `crossterm` | TUI 渲染与终端事件处理 |
| `fi-code-shared` | `ulid` | 共享 DTO（Session / Message / Part 等） |
| `fi-code-utils` | `tempfile` | 测试工具库 |
| `fi-code-tests` | `cucumber`, `reqwest`, `futures` | BDD 与 E2E 测试 |
| `fi-code-desktop` (src-tauri) | `tauri`, `tauri-plugin-shell`, `tauri-build` | Tauri v2 桌面框架 |

### 前端技术栈（Desktop）

| 依赖 | 用途 |
|------|------|
| `react` / `react-dom` | UI 框架 |
| `vite` | 构建工具 |
| `typescript` | 类型系统 |
| `tailwindcss` / `postcss` / `autoprefixer` | 样式方案 |
| `zustand` | 状态管理 |
| `@tauri-apps/api` / `@tauri-apps/cli` | Tauri v2 桌面应用框架 |

---

## 3. 代码组织与模块划分

本项目采用 **Cargo Workspace** 结构，按职责拆分为多个 Crate：

```
.
├── Cargo.toml              # Workspace 定义
├── crates/
│   ├── core/               # 核心库（fi-code-core）：所有业务逻辑
│   │   └── src/
│   │       ├── lib.rs              # 模块聚合与公共导出
│   │       ├── agent/              # Agent 核心循环、Prompt 管理、Runner、Profile
│   │       │   ├── agent.rs        # LoopState, run_one_turn(), agent_loop()
│   │       │   ├── runner.rs       # AgentRunner：可复用的 Agent 循环抽象
│   │       │   ├── prompt.rs       # PromptBuilder：系统提示词组装（内嵌 prompt_template.md）
│   │       │   └── profile.rs      # AgentProfile + ToolFilter：Build/Plan 行为配置
│   │       ├── commands/           # Slash 命令（/commit、/models 等）
│   │       ├── config/             # 配置加载、JSONC 解析、环境变量占位符、热重载
│   │       │   ├── presets.rs      # 预设 Provider 合并（openai/anthropic/glm/kimi/deepseek/qwen）
│   │       │   └── models.rs       # 配置 DTO 定义
│   │       ├── mcp/                # MCP 协议支持（types/client/transport/manager）
│   │       │   ├── manager.rs      # 多服务器管理、工具缓存、自动重连
│   │       │   ├── transport.rs    # stdio / HTTP 传输实现
│   │       │   └── types.rs        # JSON-RPC + MCP 协议类型
│   │       ├── permission/         # 权限风险分级与交互式确认
│   │       ├── provider/           # 模型对接（OpenAI / Anthropic / Mock）
│   │       │   ├── base_client.rs  # AIClient trait、Chunk 定义、send_with_retry()
│   │       │   ├── provider.rs     # Provider：模型解析、环境变量优先、客户端工厂
│   │       │   └── client/         # openapi_client.rs / anthropic_client.rs
│   │       ├── server/             # HTTP Server（API / SSE / RPC / Session）
│   │       │   ├── api/            # chat_api, file_api, log_api, session_api
│   │       │   └── transport/      # rpc.rs, sse.rs
│   │       ├── session/            # Session 与 Message 管理、JSONL 持久化
│   │       ├── skills/             # Skills 扫描、注册、加载
│   │       ├── tools/              # 工具注册表、底层实现、任务管理
│   │       │   ├── basic_tools.rs  # safe_path、bash、read/write/edit、git、grep、glob
│   │       │   ├── tools_registry.rs # HashMap 注册中心、动态 Schema 生成
│   │       │   └── task/           # TaskManager、handle_task_plan 执行
│   │       ├── tui_event.rs        # AppEvent / CardAction 等 TUI 事件枚举
│   │       ├── theme_preset.rs     # 主题预设定义
│   │       └── utils/              # 日志宏、日志存储、工作目录管理
│   ├── cli/                # CLI 二进制入口（fi-code-cli）
│   │   └── src/
│   │       ├── main.rs
│   │       ├── entry.rs            # 程序入口调度：CLI / TUI / Server 模式路由
│   │       └── cli_args.rs         # clap 参数定义
│   ├── tui/                # TUI 二进制入口（fi-code-tui）
│   │   └── src/main.rs
│   ├── server/             # Server 二进制入口（fi-code-server）
│   │   └── src/main.rs
│   ├── shared/             # 共享 DTO 与常量（fi-code-shared）
│   │   └── src/
│   │       ├── dto.rs              # Message, Part, Role, SseEvent, ThemePreset 等
│   │       ├── enums.rs            # 共享枚举
│   │       ├── constants.rs        # 常量定义
│   │       └── preset_themes.json  # 预设主题 JSON 数据
│   └── utils/              # 测试工具库（fi-code-utils）
│       └── src/main.rs
├── src-tauri/              # Tauri Desktop 应用（fi-code-desktop）
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   └── src/
│       ├── main.rs         # 二进制入口（隐藏 Windows 控制台窗口）
│       ├── lib.rs          # Tauri app 构建、3 个 command 暴露、SidecarManager 状态
│       └── sidecar.rs      # SidecarManager：spawn fi-code --server、TCP 探活、kill
├── frontend/               # Tauri 前端（React + Vite + Tailwind）
│   ├── src/
│   │   ├── App.tsx
│   │   ├── components/     # ChatPanel, InputBox, MessageBubble, LogPanel 等
│   │   ├── hooks/          # useClient, useSidecar, useChatStream, useTheme
│   │   ├── services/       # ApiClient, chat, session, model, command, file
│   │   ├── stores/         # Zustand appStore
│   │   ├── types/          # api.ts, theme.ts, events.ts
│   │   └── themes/         # 主题预设（default, light, monokai）
│   ├── package.json
│   └── vite.config.ts
└── tests/                  # 独立测试 Crate（fi-code-tests）
    ├── e2e/                # E2E 测试（CLI / TUI 流程）
    ├── bdd/                # Cucumber BDD 测试
    │   ├── features/       # 6 个 Gherkin 特性文件
    │   └── steps/          # 步骤实现
    └── Cargo.toml
```

### 关键设计原则

- **`crates/core`** 是唯一的业务逻辑载体，所有二进制 Crate 都依赖它。
- **`crates/shared`** 存放跨 Crate 共享的 DTO（Message、Part、SseEvent、ThemePreset 等），避免循环依赖。
- **模块导出**：每个模块的 `mod.rs` 负责声明子模块并重新导出（`pub use`）常用类型。
- **同步 I/O 在异步上下文中的使用**：`SessionManager` 内部使用同步 `std::fs` I/O，在异步上下文中通过 `tokio::task::spawn_blocking` 包裹调用。
- **全局单例**：工具注册表、Skill 注册表、MCP 管理器使用 `std::sync::LazyLock` 实现懒加载单例，并通过显式 `init_*()` 函数触发初始化。
- **Schema 生成**：工具的 JSON Schema 完全由 `ToolsRegistry` 动态生成。新增工具时只需在 `crates/core/src/tools/mod.rs` 的 `REGISTRY` 初始化闭包中注册。
- **Agent 类型系统**：采用配置驱动（Config-driven）的 `AgentProfile` 静态注册表，支持 `Build`（全功能）和 `Plan`（只读规划）两种模式。`AgentRunner` 作为纯调度器，在 `run_one_turn` 时动态查询当前 `AgentType` 对应的 `AgentProfile`，获取过滤后的 `tools_schema` 和带模式后缀的系统提示词。工具过滤采用双层防御：第一层在 LLM 请求前过滤 schema，第二层在 `execute_tool_calls` 执行时二次拦截，防止 LLM 绕过 schema 限制。
- **Agent 循环优化**：若 Turn 1 产生了文本前言且所有工具调用成功，则跳过 Turn 2 的 LLM 总结轮，直接以格式化文本（带 emoji）返回结果。
- **MCP 两步发现**：若 LLM 调用 `mcp:xxx` 且参数为空，Agent 会收集该工具的完整 input_schema，以 User 消息形式注入并重新请求 LLM。

---

## 4. 构建与运行

### 4.1 环境要求

- Rust 1.70+（建议最新 stable）
- Node.js 18+（仅构建 Desktop 前端时需要）
- 对应 AI Provider 的 API Key

### 4.2 配置方式

支持两种配置方式，**优先级：环境变量 > 配置文件 > 默认预设**。

#### 方式一：环境变量（最高优先级）

**OpenAI 兼容：**
```bash
OPENAI_API_KEY=sk-...
OPENAI_BASE_URL=https://api.openai.com/v1
OPENAI_MODEL_NAME=gpt-4o
```

**Anthropic：**
```bash
ANTHROPIC_API_KEY=sk-ant-...
ANTHROPIC_BASE_URL=https://api.anthropic.com
ANTHROPIC_MODEL_NAME=claude-3-7-sonnet-20250219
```

其他预设 Provider 支持的环境变量前缀：`GLM_*`、`KIMI_*`、`DEEPSEEK_*`、`QWEN_*` / `DASHSCOPE_*`。

#### 方式二：配置文件（环境变量不存在时自动降级）

配置文件路径（按优先级查找）：
- Linux/macOS: `~/.config/fi-code/config.jsonc` 或 `~/.config/fi-code/config.json`

**完整配置示例：**
```json
{
  "model": "openai/kimi-k2.5",
  "provider": {
    "openai": {
      "provider_type": "openai_compatible",
      "name": "My Provider",
      "options": {
        "apiKey": "{env:MY_API_KEY}",
        "baseURL": "https://api.example.com/v1",
        "timeout": 300000,
        "chunkTimeout": 10000
      },
      "models": {
        "kimi-k2.5": {
          "name": "Kimi K2.5",
          "maxTokens": 128000,
          "modalities": {
            "input": ["text", "image"],
            "output": ["text"]
          }
        }
      }
    }
  },
  "mcp": {
    "filesystem": {
      "type": "local",
      "enabled": true,
      "command": ["npx", "-y", "@modelcontextprotocol/server-filesystem", "/path"]
    }
  },
  "server": {
    "port": 4040,
    "api_token": null,
    "allowed_origins": null
  }
}
```

**特性说明：**
- `config.jsonc` 支持 `//` 和 `/* */` 注释
- `apiKey` 支持 `{env:VAR_NAME}` 占位符语法，启动时自动替换为对应环境变量值
- 预设 Provider（openai、anthropic、glm、kimi、qwen、deepseek）会自动合并到配置中
- 配置文件变更后自动热重载（500ms 防抖）

### 4.3 常用命令

```bash
# 编译全部
cargo build

# 运行 CLI（默认启动 TUI 模式）
cargo run --bin fi-code-cli

# 运行 CLI 交互模式
cargo run --bin fi-code-cli -- -i

# 运行 TUI
cargo run --bin fi-code-tui

# 运行 Server
cargo run --bin fi-code-server

# 运行测试
cargo test

# 格式化代码
cargo fmt

# Clippy 静态检查
cargo clippy

# Desktop 开发（需先安装 Tauri CLI）
cd frontend && npm install
cargo tauri dev

# Desktop 构建
cargo tauri build
```

### 4.4 Desktop 应用构建说明

Desktop 采用 **"Tauri 壳 + 嵌入式 Sidecar"** 架构：

1. **Tauri 窗口** 托管 React 前端（WebView）。
2. **前端** 通过标准 `fetch` 与后端通信，支持两种模式：
   - **Standalone**：前端调用 Tauri command `start_sidecar`，Tauri 在后台启动 `fi-code --server --port 4040`，前端通过 `127.0.0.1:4040` 访问。
   - **Remote**：前端直接连接外部 `fi-code-server` 实例。
3. **开发流程**：先 `cargo build` 构建出 `fi-code` 二进制（Sidecar 依赖），再 `cargo tauri dev`。
4. **生产构建**：`cargo tauri build` 会先将前端构建到 `frontend/dist`，再打包为 `deb`（Linux）、`dmg`（macOS）、`msi`（Windows）。

### 4.5 运行时的数据存储

会话数据以 `.jsonl` 格式保存在平台配置目录下：
- **Linux**: `~/.config/fi-code/sessions/`
- **macOS**: `~/Library/Application Support/fi-code/sessions/`
- **Windows**: `%APPDATA%\fi-code\sessions\`

配置文件保存在同一配置目录下。

日志文件持久化到 `~/.config/fi-code/logs/`（由 `LogFileWriter` 异步写入）。

---

## 5. 测试策略与现状

### 测试分层

| 层级 | 位置 | 说明 |
|------|------|------|
| **单元测试** | `crates/core/src/**/mod.rs` 或同名文件内的 `#[cfg(test)]` | 覆盖工具调用、权限校验、Session 创建/加载/追加、配置解析、MCP 类型、Skills 扫描等。core 约 126 个，tui 约 110 个 |
| **E2E 测试** | `tests/e2e/` | CLI 二进制（help、version、models、single command、server subcommand）和 TUI 流程测试 |
| **BDD 测试** | `tests/bdd/` | 基于 Cucumber 的行为驱动测试，覆盖 Agent 工具调用、Agent 类型过滤、Skills、Slash 命令、流式输出、任务拆分、日志窗口等 7 个场景 |

### E2E / BDD 测试目标

`tests/Cargo.toml` 中定义了 4 个测试目标：

| 测试名 | 文件 | Harness | 说明 |
|--------|------|---------|------|
| `e2e_cli` | `e2e/cli_e2e.rs` | default | CLI `--help`、`--version`、`--models`、`-c`、`server` 子命令 |
| `e2e_tui` | `e2e/tui_e2e.rs` | default | TUI `--help`、`--version`、后端服务启动 |
| `tui_flow_e2e` | `e2e/tui_flow_e2e.rs` | default | 完整流程：问候语、代码编写、任务拆分、SSE 生命周期、已有会话聊天 |
| `bdd` | `bdd_test.rs` | **false** | Cucumber BDD 运行器（`max_concurrent_scenarios(1)`） |

### 运行方式

```bash
# 运行全部测试（含单元测试）
cargo test

# 运行 E2E 测试
cargo test --test e2e_cli
cargo test --test e2e_tui
cargo test --test tui_flow_e2e

# 运行 BDD 测试（独立二进制）
cargo test --test bdd
# 或直接运行
cargo run --bin bdd
```

### 当前状态

- 单元测试分布在 `crates/core/src/` 和 `crates/tui/src/` 中，总计约 236+ 个测试用例
- E2E 测试通过 `tokio::process::Command` 和 `CARGO_BIN_EXE_*` 环境变量启动真实二进制
- BDD / TUI Flow 测试在临时目录中启动 `fi_code_core::server::Server` + `Mock` Provider，在随机端口上运行
- `FI_CODE_TEST_MODE=1` 环境变量可使 TUI 模式跳过前端界面，仅启动后端服务供测试连接
- 项目**暂无 CI/CD 配置**（`.github/`、`.gitlab-ci.yml` 等均不存在）

---

## 6. 代码风格与开发约定

1. **许可证头**：**每次新建 `.rs` 源文件时，必须在文件最顶部写入 MIT 许可证头**。禁止遗漏。已有文件如未包含许可证头，应在修改时补齐。MIT 许可证头模板见第 10 节。

2. **注释风格**：项目内大量使用中文注释，并夹杂对 Rust 基础概念（`mod`、`use`、`trait`、`LazyLock`、`Box<dyn>` 等）的教学式解释。修改代码时建议保持中文注释风格。

3. **错误处理**：统一使用 `anyhow::Result` 做错误传播；模块内部对外暴露 `Result<T>` 时优先使用 `anyhow`。

4. **模块导出**：每个模块的 `mod.rs` 负责声明子模块并重新导出（`pub use`）常用类型，减少外部调用时的路径层级。

5. **全局状态**：使用 `std::sync::LazyLock` + `RwLock` 管理全局单例（如 `MCP_MANAGER`、`TASK_PROVIDER`、`EVENT_TX`）。

6. **日志宏**：项目内使用自定义日志宏 `log_info!`、`log_debug!`、`log_trace!`、`log_warn!`、`log_error!`、`log_block!`，定义在 `crates/core/src/utils/log.rs` 中。

7. **Workspace 共享依赖**：公共依赖在根 `Cargo.toml` 的 `[workspace.dependencies]` 中声明，各 Crate 按需引用。

8. **开发记录规范**：
   - 每解决一个 bug，在 `docs/buglist/buglist-YYYY-MM-DD.md` 中追加记录（解决时间精确到分钟、模块、现象、根因、修复方案、相关 Commit）。
   - 当用户明确告知是 `refactor`（重构）时，在 `docs/refactor/refactor-YYYY-MM-DD.md` 中追加记录（处理时间精确到分钟、模块、重构动机、具体改动、预期收益、相关 Commit）。

9. **控制嵌套深度**：函数内部的嵌套层次（`if`、`match`、`loop`、`for` 等）不得超过 3 层。超过时必须抽取为独立的子函数，以保持可读性。

10. **优先使用卫子句**：处理异常或边界场景时，优先使用卫子句（Guard Clause）提前返回，避免深层 `if-else` 嵌套，保持核心逻辑在顶层可见。

11. **禁止魔法值**：代码中不允许出现未命名的字面常量（魔法值）。所有具有业务含义的常量必须定义为具名常量（`const`）或枚举（`enum`），并在使用时引用其名称。

12. **复杂类型必须使用类型别名**：如果某个复杂类型（如 `Arc<Mutex<HashMap<String, Vec<u8>>>>`）在同一模块或项目中出现 **3 次及以上**，必须定义 `type` 别名，提高可读性和维护性。

13. **调试与 Bug 修复规范**：
    - **诊断日志优先**：若无法通过用户反馈 + 代码阅读确定 Bug 根因，**禁止盲目修改代码**。必须先追加诊断日志，让用户复现并收集日志，再基于日志数据分析定位根因。
    - **用户驱动修复**：修复方案必须经用户确认日志证据后再实施，避免猜测式修复导致代码污染。
    - **最小复现**：构建最小复现用例，隔离变量，排除干扰。
    - **假设验证**：每个修复必须有明确假设，通过测试/日志验证。
    - **回滚能力**：所有修改应能通过 `git checkout` 快速回滚。

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
- **Allow（直接放行）**：`read`、`read_file`、`grep`、`glob`、`git_status`、`git_log`、`git_diff` 等只读操作。
- **Ask（交互确认）**：其他工具（如 `write`、`edit`、`web_fetch`、`git_add`、`git_commit`）以及安全的 `bash` 命令会提示用户输入 `Yes/No`。

### 7.4 Agent 类型执行时过滤（二次防线）
`execute_tool_calls` 在运行时根据当前 `AgentType` 进行二次拦截：
- **Plan Agent**：仅允许 `read`、`grep`、`glob`、`git_status`、`git_log`、`git_diff`、`web_fetch`、`create_task_plan`、`handle_task_plan`。若 LLM 绕过 schema 限制调用被禁止的工具（如 `bash`、`write`、`edit`），`execute_tool_calls` 会返回 `ToolError`，并通过 SSE 实时通知客户端。
- **Build Agent**：允许所有工具，无二次拦截。
此机制作为 schema 过滤的补充防线，防止 LLM 通过构造非法 `ToolUse` 绕过白名单。

### 7.5 输出截断
所有工具返回内容均做了 `chars().take(50000)` 截断，防止超大输出一次性撑爆 LLM 上下文。

---

## 8. 设计文档与项目知识库

项目的设计决策、实现计划和 Bug 记录统一存放在 `docs/` 目录下：

```
docs/
├── buglist/           # Bug 记录（按日期拆分）
│   └── buglist-2025-05-14.md
├── refactor/          # 重构记录（按日期拆分）
│   └── refactor-2026-05-14*.md
└── superpowers/
    ├── plans/         # 28 份实现计划（YYYY-MM-DD-特性名.md）
    └── specs/         # 24 份设计规格书（YYYY-MM-DD-特性名-design.md）
```

**注意**：`docs/superpowers/` 中的计划与规格书是项目演进的重要参考，修改相关模块前建议先查阅对应文档。`README.md` 中的项目结构图相对简化，实际代码组织以本文件第 3 节为准。

---

## 9. 给 AI 助手的快速 Checklist

- [ ] **所有新增和修改的代码必须写中文注释**，解释设计意图、非 obvious 的逻辑、以及关键决策点。禁止无注释的代码提交。
- [ ] **必须补充测试**。新增功能要补充单元测试；涉及核心流程（Agent 循环、SSE 流、工具调用、会话管理、配置加载）的修改要同步补充 BDD 或 E2E 测试。
- [ ] **每解决一个 bug，必须在 `docs/buglist/buglist-YYYY-MM-DD.md` 中追加一条记录**，按日期拆分到不同文件。描述 bug 的根因和解决方案，并**记录解决时间（精确到分钟）**。格式为：解决时间、模块、现象、根因、修复方案、相关 Commit。
- [ ] **当用户明确告知是 `refactor`（重构）时，必须在 `docs/refactor/refactor-YYYY-MM-DD.md` 中追加一条记录**，按日期拆分到不同文件。描述重构的动机、范围、具体改动和预期收益，并**记录处理时间（精确到分钟）**。格式为：处理时间、模块、重构动机、具体改动、预期收益、相关 Commit。
- [ ] 修改代码后运行 `cargo test` 确保全部通过。
- [ ] 新增工具时记得在 `crates/core/src/tools/mod.rs` 的 `REGISTRY` 中注册，并补充单元测试。
- [ ] 涉及文件系统或 Bash 的操作必须复用现有的 `safe_path` / 权限检查 / 超时 / 输出截断机制。
- [ ] 新增或修改 Agent 类型（`AgentType`）、Agent 画像（`AgentProfile`）或工具过滤逻辑时，同步更新 `crates/core/src/agent/profile.rs` 中的静态注册表，并确保双层过滤（schema 过滤 + 执行时过滤）均生效。同步更新本文第 3 节和第 7.4 节。
- [ ] 保持中文注释风格，对新增的 Rust 语法或设计模式可适当补充说明。
- [ ] `Cargo.toml` 或环境变量相关变更需在本文对应章节同步更新。
- [ ] 新增或修改配置模块功能时，同步更新本文第 4.1 节（配置方式）的示例和说明。
- [ ] 配置文件格式变更时，同步更新设计文档 `docs/` 中的相关文档。
- [ ] **新建任何 `.rs` 文件时，必须在文件顶部写入 MIT 许可证头**（模板见第 10 节）。
- [ ] 修改 TUI / Desktop 前端时，同步检查 `frontend/` 和 `src-tauri/` 的对应逻辑。

---

## 10. MIT 许可证头模板

所有新建或修改的 Rust 源文件（`.rs`）均须在文件最顶部粘贴以下许可证头：

```rust
// MIT License
// Copyright (c) 2025 fi-code contributors
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.
```
