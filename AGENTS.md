<!-- From: /home/nan/fi-code/AGENTS.md -->
# AGENTS.md —— fi-code 项目指南

> 本文件面向 AI 编程助手。如果你刚刚拿到这个项目，请先阅读本文以了解代码结构、构建方式、开发约定和安全注意事项。

---

## 1. 项目概览

**fi-code** 是一个基于 Rust 构建的终端 AI Coding Agent。它通过多模式交互（REPL、TUI、HTTP Server、Desktop）与用户协作，支持多轮对话、工具调用（文件读写、Bash 执行、网页抓取、代码搜索、Git 操作等）、任务拆分、会话持久化以及 MCP（Model Context Protocol）扩展。

- **语言**：Rust（Edition 2021）
- **版本**：`0.1.0`
- **包结构**：Cargo Workspace，核心逻辑在 `fi-code-core`，多个前端入口（CLI / TUI / Server / Desktop）
- **运行时**：基于 `tokio` 的异步运行时

### 核心能力

1. **多模式交互**：
   - **CLI REPL**：传统命令行交互模式（`fi-code-cli -i`）
   - **TUI**：基于 `ratatui` 的全终端界面（`fi-code-tui`）
   - **HTTP Server**：REST API + SSE 流式响应（`fi-code-server` 或 `fi-code-cli server`）
   - **Desktop**：基于 Tauri v2 的桌面应用（`fi-code-desktop`）

2. **模型对接**：统一封装了 OpenAI 兼容接口与 Anthropic 接口，支持流式 SSE 响应解析。

3. **工具调用**：内置 15+ 个工具，Agent 可根据模型返回的 `ToolUse` 自动执行并回传结果：
   - 基础工具：`bash`、`read`、`write`、`edit`、`web_fetch`、`grep`、`glob`
   - Git 工具：`git`、`git_status`、`git_diff`、`git_add`、`git_commit`、`git_log`、`git_worktree`
   - 任务工具：`create_task_plan`、`handle_task_plan`
   - 交互工具：`ask_for_question`
   - Skill 工具：`use_skill`
   - MCP 工具：动态加载，以 `mcp:` 为前缀

4. **会话持久化**：采用 JSONL（JSON Lines）格式将会话增量写入本地磁盘，支持中断后恢复。

5. **权限校验**：对 Bash 等高危操作进行风险分级（Allow / Ask / Deny），拦截 `sudo`、`rm -rf` 及常见注入攻击。

6. **配置管理**：支持通过 `~/.config/fi-code/config.json` 或 `config.jsonc` 管理模型和 Provider 设置，支持 JSONC 注释、环境变量占位符（`{env:VAR_NAME}`）以及文件系统事件热重载（500ms 防抖）。

7. **MCP 支持**：完整实现 Model Context Protocol，支持多服务器管理（stdio / HTTP 传输）。

8. **Skills 系统**：可扩展的 Skill 注册与加载机制，Agent 可通过 `use_skill` 工具按需加载项目内的 Skill 指令。

---

## 2. 技术栈与关键依赖

### Rust 依赖（Workspace 级别）

| 依赖 | 用途 |
|------|------|
| `tokio` | 异步运行时 |
| `reqwest` | HTTP 客户端，支持 SSE 流式请求 |
| `serde` / `serde_json` | 序列化与反序列化 |
| `anyhow` | 简化错误传播 |
| `colored` | 终端彩色输出 |
| `dotenvy` | 加载 `.env` 环境变量 |
| `ulid` | 生成 Session / Message ID |
| `directories` / `dirs` | 解析平台相关的配置目录 |
| `notify` | 配置文件热重载的文件系统事件监听 |
| `jsonc-parser` | 解析带注释的 JSONC 配置文件 |
| `html2md` | 网页 HTML 转 Markdown |
| `regex` | `grep` 工具的正则匹配 |
| `async-trait` | 异步 trait 支持 |
| `futures` / `bytes` / `rand` | 流处理、字节操作、随机数 |
| `clap` | 命令行参数解析 |
| `ratatui` / `crossterm` | TUI 渲染与终端事件处理 |
| `axum` / `tower-http` | HTTP Server 与 CORS |
| `chrono` | 时间戳处理 |
| `walkdir` / `glob` | 目录遍历与文件匹配 |
| `similar` | 文本 diff |
| `cucumber` | BDD 行为驱动测试 |
| `insta` | 快照测试 |

### 前端技术栈（Desktop）

| 依赖 | 用途 |
|------|------|
| `react` / `react-dom` | UI 框架 |
| `vite` | 构建工具 |
| `typescript` | 类型系统 |
| `tailwindcss` / `postcss` / `autoprefixer` | 样式方案 |
| `zustand` | 状态管理 |
| `@tauri-apps/api` / `@tauri-apps/cli` | Tauri v2 桌面应用框架 |

### 开发依赖

- `wiremock`（HTTP Mock）
- `tempfile`（临时目录测试）

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
│   │       ├── entry.rs            # 程序入口调度：CLI / TUI / Server 模式路由
│   │       ├── agent/              # Agent 核心循环、Prompt 管理、Runner
│   │       ├── commands/           # Slash 命令（/commit、/models 等）
│   │       ├── config/             # 配置加载、JSONC 解析、环境变量占位符、热重载
│   │       ├── mcp/                # MCP 协议支持（types/client/transport/manager）
│   │       ├── permission/         # 权限风险分级与交互式确认
│   │       ├── provider/           # 模型对接（OpenAI / Anthropic / Mock）
│   │       ├── server/             # HTTP Server（API / SSE / RPC / Session）
│   │       ├── session/            # Session 与 Message 管理、JSONL 持久化
│   │       ├── skills/             # Skills 扫描、注册、加载
│   │       ├── tools/              # 工具注册表、底层实现、任务管理
│   │       ├── tui/                # 终端 UI（ratatui）
│   │       └── utils/              # 日志、CLI 参数、工作目录、日志存储
│   ├── cli/                # CLI 二进制入口（fi-code-cli）
│   │   └── src/main.rs
│   ├── tui/                # TUI 二进制入口（fi-code-tui）
│   │   └── src/main.rs
│   ├── server/             # Server 二进制入口（fi-code-server）
│   │   └── src/main.rs
│   ├── shared/             # 共享 DTO 与常量（fi-code-shared）
│   └── utils/              # 测试工具库（fi-code-utils）
├── src-tauri/              # Tauri Desktop 应用（fi-code-desktop）
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   └── src/
│       ├── main.rs
│       ├── lib.rs
│       └── sidecar.rs
├── frontend/               # Tauri 前端（React + Vite + Tailwind）
│   ├── src/
│   │   ├── App.tsx
│   │   ├── main.tsx
│   │   └── vite-env.d.ts
│   ├── package.json
│   ├── vite.config.ts
│   ├── tailwind.config.js
│   └── tsconfig.json
└── tests/                  # 独立测试 Crate（fi-code-tests）
    ├── e2e/                # E2E 测试（CLI / TUI）
    ├── bdd/                # Cucumber BDD 测试
    │   ├── features/       # Gherkin 特性文件
    │   └── steps/          # 步骤实现
    └── Cargo.toml
```

### 关键设计原则

- **`crates/core`** 是唯一的业务逻辑载体，所有二进制 Crate 都依赖它。
- **模块导出**：每个模块的 `mod.rs` 负责声明子模块并重新导出（`pub use`）常用类型。
- **同步 I/O 在异步上下文中的使用**：`SessionManager` 内部使用同步 `std::fs` I/O，在异步上下文中通过 `tokio::task::spawn_blocking` 包裹调用。
- **全局单例**：工具注册表和 Skill 注册表使用 `std::sync::LazyLock` 实现懒加载单例，并通过显式 `init_*()` 函数触发初始化。
- **Schema 生成**：工具的 JSON Schema 完全由 `ToolsRegistry` 动态生成，新增工具时只需在 `crates/core/src/tools/mod.rs` 的 `REGISTRY` 初始化闭包中注册。

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

### 4.4 运行时的数据存储

会话数据以 `.jsonl` 格式保存在平台配置目录下：
- **Linux**: `~/.config/fi-code/sessions/`
- **macOS**: `~/Library/Application Support/fi-code/sessions/`
- **Windows**: `%APPDATA%\fi-code\sessions\`

配置文件保存在同一配置目录下。

---

## 5. 测试策略与现状

### 测试分层

| 层级 | 位置 | 说明 |
|------|------|------|
| **单元测试** | `crates/core/src/**/mod.rs` 或同名文件内的 `#[cfg(test)]` | 覆盖工具调用、权限校验、Session 创建/加载/追加、配置解析、MCP 类型、Skills 扫描等 |
| **E2E 测试** | `tests/e2e/` | CLI 二进制（help、version、models、single command、server subcommand）和 TUI 流程测试 |
| **BDD 测试** | `tests/bdd/` | 基于 Cucumber 的行为驱动测试，覆盖 Agent 工具调用、Skills、Slash 命令、流式输出、任务拆分、日志窗口等场景 |

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

- 单元测试分布在 `crates/core/src/` 各模块中，约 190+ 个测试用例
- E2E 测试验证 CLI 和 Server 的端到端行为
- BDD 测试使用 Gherkin 语法描述用户场景，在临时目录中启动 Mock Provider 和 Server 进行验证
- `FI_CODE_TEST_MODE` 环境变量可使 TUI 模式跳过前端界面，仅启动后端服务供测试连接

---

## 6. 代码风格与开发约定

1. **许可证头**：**每次新建 `.rs` 源文件时，必须在文件最顶部写入 MIT 许可证头**。禁止遗漏。已有文件如未包含许可证头，应在修改时补齐。MIT 许可证头模板见第 10 节。

2. **注释风格**：项目内大量使用中文注释，并夹杂对 Rust 基础概念（`mod`、`use`、`trait`、`LazyLock`、`Box<dyn>` 等）的教学式解释。修改代码时建议保持中文注释风格。

3. **错误处理**：统一使用 `anyhow::Result` 做错误传播；模块内部对外暴露 `Result<T>` 时优先使用 `anyhow`。

4. **模块导出**：每个模块的 `mod.rs` 负责声明子模块并重新导出（`pub use`）常用类型，减少外部调用时的路径层级。

5. **全局状态**：使用 `std::sync::LazyLock` + `RwLock` 管理全局单例（如 `MCP_MANAGER`、`TASK_PROVIDER`、`EVENT_TX`）。

6. **日志宏**：项目内使用自定义日志宏 `log_info!`、`log_debug!`、`log_trace!`，定义在 `crates/core/src/utils/log.rs` 中。

7. **Workspace 共享依赖**：公共依赖在根 `Cargo.toml` 的 `[workspace.dependencies]` 中声明，各 Crate 按需引用。

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

### 7.4 输出截断
所有工具返回内容均做了 `chars().take(50000)` 截断，防止超大输出一次性撑爆 LLM 上下文。

---

## 8. 设计文档参考

- `docs/session-desgin.md`：详细的 Session-Message-Part 持久化系统设计，包含 JSONL 格式规范、存储布局、`SessionManager` API、错误处理策略与测试策略。

---

## 9. 给 AI 助手的快速 Checklist

- [ ] **所有新增和修改的代码必须写中文注释**，解释设计意图、非 obvious 的逻辑、以及关键决策点。禁止无注释的代码提交。
- [ ] **必须补充测试**。新增功能要补充单元测试；涉及核心流程（Agent 循环、SSE 流、工具调用、会话管理、配置加载）的修改要同步补充 BDD 或 E2E 测试。
- [ ] **每解决一个 bug，必须在 `docs/buglist.md` 中追加一条记录**，描述 bug 的根因和解决方案。格式为：日期、模块、现象、根因、修复方案、相关 Commit。
- [ ] 修改代码后运行 `cargo test` 确保全部通过。
- [ ] 新增工具时记得在 `crates/core/src/tools/mod.rs` 的 `REGISTRY` 中注册，并补充单元测试。
- [ ] 涉及文件系统或 Bash 的操作必须复用现有的 `safe_path` / 权限检查 / 超时 / 输出截断机制。
- [ ] 保持中文注释风格，对新增的 Rust 语法或设计模式可适当补充说明。
- [ ] `Cargo.toml` 或环境变量相关变更需在本文对应章节同步更新。
- [ ] 新增或修改配置模块功能时，同步更新本文第 4.1 节（配置方式）的示例和说明。
- [ ] 配置文件格式变更时，同步更新设计文档 `docs/` 中的相关文档。
- [ ] **新建任何 `.rs` 文件时，必须在文件顶部写入 MIT 许可证头**（模板见第 10 节）。

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
