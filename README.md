# fi-code

一个基于 Rust 构建的终端 AI Coding Agent CLI 程序，通过 REPL 或 TUI 方式与用户交互，支持多轮对话、工具调用、会话持久化以及 MCP 协议扩展。

## 特性

- **🤖 多模型支持**：统一封装 OpenAI 兼容接口与 Anthropic 接口，支持流式 SSE 响应
- **🔧 工具调用**：内置 6 个工具（`bash`、`read`、`write`、`edit`、`web_fetch`、`grep`），Agent 可根据模型返回自动执行并回传结果
- **💬 会话持久化**：采用 JSON Lines 格式将会话增量写入本地磁盘，支持中断后恢复
- **🖥️ 双模交互**：支持传统 REPL 交互与基于 `ratatui` 的全终端 TUI 界面
- **🛡️ 权限校验**：对 Bash 等高危操作进行风险分级（Allow / Ask / Deny），拦截 `sudo`、`rm -rf` 及常见注入攻击
- **⚙️ 灵活配置**：支持 `~/.config/fi-code/config.json` 或 `config.jsonc`，支持注释、环境变量占位符及热重载
- **🔗 MCP 支持**：已实现 Model Context Protocol，支持多服务器管理（stdio/HTTP 传输）
- **📦 Skills 系统**：支持可扩展的 Skill 注册与加载机制

## 快速开始

### 环境要求

- [Rust](https://rustup.rs/) 1.70+（推荐最新稳定版）
- 对应的 AI Provider API Key

### 安装

```bash
# 克隆仓库
git clone <repository-url>
cd fi-code

# 编译
cargo build --release

# 运行（开发模式）
cargo run -- --help
```

### 配置

#### 方式一：环境变量（最高优先级）

**OpenAI 兼容：**
```bash
export OPENAI_API_KEY=sk-...
export OPENAI_BASE_URL=https://api.openai.com/v1
export OPENAI_MODEL_NAME=gpt-4o
```

**Anthropic：**
```bash
export ANTHROPIC_API_KEY=sk-ant-...
export ANTHROPIC_BASE_URL=https://api.anthropic.com
export ANTHROPIC_MODEL_NAME=claude-3-7-sonnet-20250219
```

#### 方式二：配置文件

配置文件路径（按优先级查找）：
- Linux/macOS: `~/.config/fi-code/config.jsonc` 或 `~/.config/fi-code/config.json`

示例：
```json
{
  "model": "my-model",
  "provider": {
    "openai": {
      "name": "My Provider",
      "options": {
        "apiKey": "{env:MY_API_KEY}",
        "baseURL": "https://api.example.com/v1"
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

支持 `//` 和 `/* */` 注释，`apiKey` 支持 `{env:VAR_NAME}` 占位符语法。

### 使用

```bash
# 交互式 REPL 模式
cargo run -- -i

# TUI 全终端界面模式
cargo run -- --tui

# 执行单条命令
cargo run -- -c "帮我写一个 Rust Hello World"

# 查看已配置的模型
cargo run -- --models

# 查看会话列表
cargo run -- -s

# 指定工作目录
cargo run -- -i -w /path/to/project
```

## 项目结构

```
src/
├── main.rs                 # 程序入口
├── agent/                  # Agent 核心循环与提示词管理
├── provider/               # 模型对接（OpenAI / Anthropic）
├── session/                # 会话与消息管理
├── tools/                  # 工具注册与实现
├── config/                 # 配置加载与热重载
├── permission/             # 权限风险分级
├── tui/                    # 终端用户界面（ratatui）
├── mcp/                    # MCP 协议支持
├── skills/                 # Skills 系统
├── commands/               # 斜杠命令
└── utils/                  # 通用工具
```

## 内置工具

| 工具 | 说明 | 风险等级 |
|------|------|----------|
| `bash` | 执行 shell 命令 | Ask（危险命令 Deny） |
| `read` / `read_file` | 读取文件内容 | Allow |
| `write` | 写入文件 | Ask |
| `edit` | 编辑文件 | Ask |
| `web_fetch` | 抓取网页并转为 Markdown | Ask |
| `grep` | 正则搜索文件内容 | Allow |

## 安全机制

- **路径逃逸防护**：所有文件操作通过 `safe_path` 检查，确保不超出工作目录
- **Bash 沙箱**：清除环境变量，仅保留最小必要变量，120 秒超时
- **权限分级**：Deny（直接拒绝危险命令）、Ask（交互确认）、Allow（只读操作直接放行）
- **输出截断**：工具返回内容限制在 50,000 字符以内，防止撑爆上下文

## TUI 快捷键

在 TUI 模式下，可使用以下快捷键：

| 快捷键 | 功能 |
|--------|------|
| `Tab` / `Shift+Tab` | 切换焦点区域 |
| `Ctrl+C` | 停止生成 / 退出程序 |
| `Ctrl+B` | 打开/关闭左侧文件抽屉 |
| `Ctrl+H` | 打开/关闭右侧会话历史抽屉 |
| `Ctrl+M` | 打开模型选择下拉框 |
| `Ctrl+T` | 切换主题 |
| `Ctrl+N` | 新建会话 |
| `Enter` | 发送消息 |
| `Shift+Enter` | 输入框内换行 |
| `Esc` | 关闭抽屉/下拉框/返回主区域 |
| `Ctrl+Up` / `PageUp` | 聊天区向上滚动 |
| `Ctrl+Down` / `PageDown` | 聊天区向下滚动 |

## 开发

```bash
# 运行测试
cargo test

# 格式化代码
cargo fmt

# Clippy 静态检查
cargo clippy
```

## 技术栈

| 依赖 | 用途 |
|------|------|
| `tokio` | 异步运行时 |
| `reqwest` | HTTP 客户端，SSE 流式请求 |
| `serde` / `serde_json` | 序列化与反序列化 |
| `anyhow` | 错误处理 |
| `rustyline` | 终端行读取与历史记录 |
| `ratatui` / `crossterm` | TUI 渲染与终端事件 |
| `colored` | 终端彩色输出 |
| `clap` | 命令行参数解析 |
| `notify` | 配置文件热重载 |
| `regex` | 正则匹配 |

## 会话存储

会话数据以 `.jsonl` 格式保存在平台配置目录下：
- **Linux**: `~/.config/fi-code/sessions/`
- **macOS**: `~/Library/Application Support/fi-code/sessions/`
- **Windows**: `%APPDATA%\fi-code\sessions\`

## 许可证

本项目采用 [MIT License](./LICENSE) 开源许可。

Copyright (c) 2025 fi-code contributors.

---

> **提示**：本项目处于早期开发阶段，API 和配置格式可能会发生变化。
