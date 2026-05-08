<p align="center">
  <a href="README.md">English</a> |
  <a href="README.zh_CN.md">中文</a> |
  <a href="README.fr.md">Français</a> |
  <a href="README.ja.md">日本語</a> |
  <a href="README.de.md">Deutsch</a>
</p>

# fi-code

A terminal AI Coding Agent CLI built in Rust, interacting with users via REPL or TUI. It supports multi-turn conversations, tool calls, session persistence, and MCP protocol extensions.

## Features

- **🤖 Multi-Model Support**: Unified OpenAI-compatible and Anthropic interfaces with streaming SSE responses
- **🔧 Tool Calling**: 6 built-in tools (`bash`, `read`, `write`, `edit`, `web_fetch`, `grep`). The Agent can auto-execute based on model responses and return results
- **💬 Session Persistence**: Sessions are incrementally written to local disk in JSON Lines format, supporting resume after interruption
- **🖥️ Dual-Mode Interaction**: Traditional REPL interaction and full-terminal TUI interface powered by `ratatui`
- **🛡️ Permission Validation**: Risk grading for high-risk operations like Bash (Allow / Ask / Deny), intercepting `sudo`, `rm -rf`, and common injection attacks
- **⚙️ Flexible Configuration**: Supports `~/.config/fi-code/config.json` or `config.jsonc`, with comments, environment variable placeholders, and hot-reload
- **🔗 MCP Support**: Model Context Protocol implemented, supporting multi-server management (stdio/HTTP transport)
- **📦 Skills System**: Extensible Skill registration and loading mechanism

## Quick Start

### Requirements

- [Rust](https://rustup.rs/) 1.70+ (latest stable recommended)
- Corresponding AI Provider API Key

### Installation

```bash
# Clone the repository
git clone <repository-url>
cd fi-code

# Build
cargo build --release

# Run (development mode)
cargo run -- --help
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

#### Method 2: Configuration File

Config file paths (searched in priority order):
- Linux/macOS: `~/.config/fi-code/config.jsonc` or `~/.config/fi-code/config.json`

Example:
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

Supports `//` and `/* */` comments. `apiKey` supports `{env:VAR_NAME}` placeholder syntax.

### Usage

```bash
# Interactive REPL mode
cargo run -- -i

# TUI full-terminal interface mode
cargo run -- --tui

# Execute a single command
cargo run -- -c "Write me a Rust Hello World"

# View configured models
cargo run -- --models

# View session list
cargo run -- -s

# Specify working directory
cargo run -- -i -w /path/to/project
```

## Project Structure

```
src/
├── main.rs                 # Program entry
├── agent/                  # Agent core loop and prompt management
├── provider/               # Model integration (OpenAI / Anthropic)
├── session/                # Session and message management
├── tools/                  # Tool registry and implementation
├── config/                 # Config loading and hot-reload
├── permission/             # Permission risk grading
├── tui/                    # Terminal UI (ratatui)
├── mcp/                    # MCP protocol support
├── skills/                 # Skills system
├── commands/               # Slash commands
└── utils/                  # Common utilities
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

## Security Mechanisms

- **Path Escape Protection**: All file operations go through `safe_path` checks to ensure they don't exceed the working directory
- **Bash Sandbox**: Clears environment variables, keeps only minimal necessary variables, 120-second timeout
- **Permission Grading**: Deny (directly reject dangerous commands), Ask (interactive confirmation), Allow (read-only operations pass directly)
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
# Run tests
cargo test

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
| `rustyline` | Terminal line reading and history |
| `ratatui` / `crossterm` | TUI rendering and terminal events |
| `colored` | Terminal colored output |
| `clap` | Command-line argument parsing |
| `notify` | Config file hot-reload |
| `regex` | Regex matching |

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
