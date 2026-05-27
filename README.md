<p align="center">
  <a href="README.md">English</a> |
  <a href="README.zh_CN.md">中文</a> |
  <a href="README.fr.md">Français</a> |
  <a href="README.ja.md">日本語</a> |
  <a href="README.de.md">Deutsch</a>
</p>

# fi-code

A terminal AI Coding Agent built in Rust, interacting with users via REPL, TUI, HTTP Server, or Desktop app. It supports multi-turn conversations, tool calls, session persistence, task planning, and MCP protocol extensions.

## Features

- **🤖 Multi-Model Support**: Unified OpenAI-compatible and Anthropic interfaces with streaming SSE responses and built-in retry (exponential backoff + full jitter)
- **🔧 Tool Calling**: 20 built-in tools including `bash`, `read`, `write`, `edit`, `web_fetch`, `grep`, `glob`, `git` (status/diff/add/commit/log), `create_task_plan`, `handle_task_plan`, `ask_for_question`, and `use_skill`. The Agent auto-executes based on model responses
- **💬 Session Persistence**: Sessions are incrementally written to local disk in JSON Lines format, supporting resume after interruption
- **🖥️ Multi-Mode Interaction**:
  - **CLI REPL**: Traditional command-line interaction (`fi-code-cli -i`)
  - **TUI**: Full-terminal interface powered by `ratatui` (`fi-code-tui`)
  - **HTTP Server**: REST API + SSE streaming (`fi-code-server` or `fi-code-cli server`)
  - **Desktop**: Tauri v2 desktop app with embedded sidecar (`fi-code-desktop`)
- **🛡️ Permission Validation**: Risk grading for high-risk operations like Bash (Allow / Ask / Deny), intercepting `sudo`, `rm -rf`, and common injection attacks
- **⚙️ Flexible Configuration**: Supports `~/.config/fi-code/config.json` or `config.jsonc`, with comments, environment variable placeholders (`{env:VAR_NAME}`), and hot-reload (500ms debounce)
- **🔗 MCP Support**: Full Model Context Protocol implementation, supporting multi-server management (stdio/HTTP transport) with auto-reconnect
- **📦 Skills System**: Extensible Skill registration and loading mechanism; Agent can load project-specific skills on demand via `use_skill`
- **📋 Task Planning**: Built-in task splitting with `create_task_plan` and `handle_task_plan` tools for complex multi-step workflows
- **🔍 Observability**: OpenTelemetry-based tracing with Langfuse integration for LLM call monitoring

## Quick Start

### Requirements

- [Rust](https://rustup.rs/) 1.70+ (latest stable recommended)
- Node.js 18+ (only needed for Desktop frontend build)
- Corresponding AI Provider API Key

### Installation

```bash
# Clone the repository
git clone <repository-url>
cd fi-code

# Build
cargo build --release
```

### Configuration

#### Method 1: Environment Variables (Highest Priority)

**OpenAI Compatible:**
```bash
export OPENAI_API_KEY=sk-...
export OPENAI_BASE_URL=https://api.openai.com/v1
export OPENAI_MODEL_NAME=gpt-4o
```

**Anthropic:**
```bash
export ANTHROPIC_API_KEY=sk-ant-...
export ANTHROPIC_BASE_URL=https://api.anthropic.com
export ANTHROPIC_MODEL_NAME=claude-3-7-sonnet-20250219
```

Other preset providers support `GLM_*`, `KIMI_*`, `DEEPSEEK_*`, `QWEN_*` / `DASHSCOPE_*` prefixes.

#### Method 2: Configuration File

Config file paths (searched in priority order):
- Linux/macOS: `~/.config/fi-code/config.jsonc` or `~/.config/fi-code/config.json`

Example:
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

Supports `//` and `/* */` comments. `apiKey` supports `{env:VAR_NAME}` placeholder syntax.

### Usage

```bash
# Interactive REPL mode
cargo run --bin fi-code-cli -- -i

# TUI full-terminal interface mode
cargo run --bin fi-code-tui

# HTTP Server mode
cargo run --bin fi-code-server
# or
cargo run --bin fi-code-cli -- server

# Execute a single command
cargo run --bin fi-code-cli -- -c "Write me a Rust Hello World"

# View configured models
cargo run --bin fi-code-cli -- --models

# View session list
cargo run --bin fi-code-cli -- -s

# Specify working directory
cargo run --bin fi-code-cli -- -i -w /path/to/project
```

### Desktop Development

```bash
# Install frontend dependencies
cd frontend && npm install

# Run in development mode (requires fi-code binary built first)
cargo tauri dev

# Build for production
cargo tauri build
```

## Project Structure

This project uses a Cargo Workspace structure:

```
.
├── Cargo.toml              # Workspace definition
├── crates/
│   ├── core/               # Core library (fi-code-core): all business logic
│   ├── cli/                # CLI binary entry (fi-code-cli)
│   ├── tui/                # TUI binary entry (fi-code-tui)
│   ├── server/             # Server binary entry (fi-code-server)
│   ├── shared/             # Shared DTOs and constants (fi-code-shared)
│   └── utils/              # Test utilities (fi-code-utils)
├── src-tauri/              # Tauri Desktop app (fi-code-desktop)
├── frontend/               # Tauri frontend (React + Vite + Tailwind)
└── tests/                  # E2E and BDD tests (fi-code-tests)
```

## Built-in Tools

| Tool | Description | Risk Level |
|------|-------------|------------|
| `bash` | Execute shell commands | Ask (Dangerous commands Deny) |
| `read` / `read_file` | Read file contents | Allow |
| `write` | Write to file | Ask |
| `edit` | Edit file | Ask |
| `web_fetch` | Fetch webpage and convert to Markdown | Ask |
| `grep` | Regex search file contents | Allow |
| `glob` | Find files by pattern | Allow |
| `git` | Execute git commands | Ask |
| `git_status` | Check git status | Allow |
| `git_diff` | Show git diff | Allow |
| `git_add` | Stage files | Ask |
| `git_commit` | Commit changes | Ask |
| `git_log` | Show git log | Allow |
| `git_worktree` | Manage git worktrees | Ask |
| `create_task_plan` | Create a task plan | Allow |
| `handle_task_plan` | Execute task plan steps | Ask |
| `ask_for_question` | Ask user a question | Allow |
| `use_skill` | Load and use a skill | Allow |
| `mcp:*` | Dynamically loaded MCP tools | Varies |

## Security Mechanisms

- **Path Escape Protection**: All file operations go through `safe_path` checks to ensure they don't exceed the working directory
- **Bash Sandbox**: Clears environment variables (blocks `LD_PRELOAD`, `BASH_ENV`), keeps only minimal `PATH` and `HOME`, with 120-second timeout
- **Permission Grading**: Deny (directly reject dangerous commands), Ask (interactive confirmation), Allow (read-only operations pass directly)
- **Agent Type Enforcement**: Plan Agent can only use read-only tools; Build Agent has full access. Runtime filtering prevents LLM from bypassing schema restrictions
- **Output Truncation**: Tool return content is limited to 50,000 characters to prevent context overflow

## TUI Shortcuts

In TUI mode, the following shortcuts are available:

| Shortcut | Function |
|----------|----------|
| `Tab` / `Shift+Tab` | Switch focus area |
| `Ctrl+C` | Stop generation / exit program |
| `Ctrl+B` | Open/close left file drawer |
| `Ctrl+H` | Open/close right session history drawer |
| `Ctrl+M` | Open model selection dropdown |
| `Ctrl+T` | Switch theme |
| `Ctrl+N` | New session |
| `Enter` | Send message |
| `Shift+Enter` | New line in input box |
| `Esc` | Close drawer/dropdown/return to main area |
| `Ctrl+Up` / `PageUp` | Scroll chat area up |
| `Ctrl+Down` / `PageDown` | Scroll chat area down |

## Development

```bash
# Run all tests (unit + E2E + BDD)
cargo test

# Run specific test targets
cargo test --test e2e_cli
cargo test --test e2e_tui
cargo test --test tui_flow_e2e
cargo test --test bdd

# Format code
cargo fmt

# Clippy static check
cargo clippy
```

## Tech Stack

| Dependency | Purpose |
|------------|---------|
| `tokio` | Async runtime |
| `reqwest` | HTTP client, SSE streaming requests |
| `serde` / `serde_json` | Serialization and deserialization |
| `anyhow` | Error handling |
| `axum` | HTTP Server framework |
| `tower-http` | HTTP CORS middleware |
| `ratatui` / `crossterm` | TUI rendering and terminal events |
| `tauri` | Desktop application framework |
| `colored` | Terminal colored output |
| `clap` | Command-line argument parsing |
| `notify` | Config file hot-reload |
| `regex` | Regex matching |
| `opentelemetry` / `opentelemetry-otlp` | Observability and tracing |

## Session Storage

Session data is saved in `.jsonl` format under the platform config directory:
- **Linux**: `~/.config/fi-code/sessions/`
- **macOS**: `~/Library/Application Support/fi-code/sessions/`
- **Windows**: `%APPDATA%\fi-code\sessions\`

## License

This project is open-sourced under the [MIT License](./LICENSE).

Copyright (c) 2025 fi-code contributors.

---

> **Note**: This project is in early development stage. APIs and configuration formats may change.
