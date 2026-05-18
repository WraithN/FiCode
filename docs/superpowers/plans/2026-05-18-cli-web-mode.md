# CLI Web 模式 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 fi-code-cli 新增 `-W` / `--web [PORT]` 参数，一键启动 Web UI（复用 Desktop 前端），同时提供 HTTP API 服务和静态文件服务。

**Architecture:** 在现有 Axum Server 基础上增加 `tower_http::services::ServeDir` 静态文件服务，CLI 检测到 `-W` 时初始化完整依赖链并启动 Server，随后调用 `open::that()` 打开浏览器。前端通过 `window.location.origin` 动态推断 API base URL。

**Tech Stack:** Rust (axum, tower-http, clap, open), TypeScript/React (Vite 构建产物)

---

## 文件结构映射

| 文件 | 职责 | 操作 |
|------|------|------|
| `frontend/src/services/apiClient.ts` | API 客户端，硬编码了 `localhost:4040` | 修改：动态 base URL |
| `crates/core/Cargo.toml` | core crate 依赖 | 修改：`tower-http` 增加 `fs` feature |
| `crates/core/src/server/server.rs` | Server 启动、Router 构建 | 修改：增加 `ServeDir` + SPA fallback |
| `crates/cli/Cargo.toml` | cli crate 依赖 | 修改：增加 `open = "5"` |
| `crates/cli/src/cli_args.rs` | CLI 参数定义 | 修改：新增 `--web` / `-W` |
| `crates/cli/src/entry.rs` | 入口调度逻辑 | 修改：新增 `start_web_mode()` |
| `tests/e2e/cli_e2e.rs` | CLI E2E 测试 | 修改：增加 `--web` 参数解析测试 |

---

## Task 1: 前端动态 Base URL

**Files:**
- Modify: `frontend/src/services/apiClient.ts:1-20`

- [ ] **Step 1: 添加 detectBaseUrl 函数**

```typescript
function detectBaseUrl(): string {
  // Tauri Desktop 环境：固定连接本地 Sidecar
  if ((window as any).__TAURI_INTERNALS__) {
    return 'http://127.0.0.1:4040';
  }
  // 浏览器环境（Web 模式）：自动推断当前地址
  return window.location.origin;
}
```

- [ ] **Step 2: 修改 ApiClient 构造函数**

将第 8 行：
```typescript
constructor(baseUrl: string = 'http://localhost:4040') {
```

改为：
```typescript
constructor(baseUrl?: string) {
  this.baseUrl = (baseUrl ?? detectBaseUrl()).replace(/\/$/, '');
}
```

- [ ] **Step 3: 运行 TypeScript 检查**

Run: `cd frontend && npx tsc --noEmit`
Expected: 0 errors

- [ ] **Step 4: 运行前端构建**

Run: `cd frontend && npm run build`
Expected: build success, `dist/` generated

- [ ] **Step 5: Commit**

```bash
git add frontend/src/services/apiClient.ts
git commit -m "feat(web-mode): frontend auto-detects API base URL from window.location"
```

---

## Task 2: Server 静态文件服务

**Files:**
- Modify: `crates/core/Cargo.toml`
- Modify: `crates/core/src/server/server.rs`

- [ ] **Step 1: 给 tower-http 添加 fs feature**

修改 `crates/core/Cargo.toml` 中 `tower-http` 行：

```toml
tower-http = { version = "0.5", features = ["cors", "fs"] }
```

- [ ] **Step 2: 添加 find_frontend_dist 辅助函数**

在 `crates/core/src/server/server.rs` 中 `AppState` 定义之前添加：

```rust
use std::path::PathBuf;

/// 从可执行文件位置向上查找 frontend/dist 目录
fn find_frontend_dist() -> Option<PathBuf> {
    let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();

    let candidates = [
        exe_dir.join("frontend/dist"),
        exe_dir.join("../frontend/dist"),
        exe_dir.join("../../frontend/dist"),
        PathBuf::from("frontend/dist"),
    ];

    for path in &candidates {
        if path.join("index.html").exists() {
            return Some(path.clone());
        }
    }
    None
}
```

- [ ] **Step 3: 在 Router 中添加静态文件服务**

修改 `Server::run()` 方法。在现有的 `let app = Router::new()` 链之后、`.layer(cors_layer(...))` 之前，插入 `.fallback_service(...)`：

```rust
    pub async fn run(self) {
        let frontend_dist = find_frontend_dist();
        if frontend_dist.is_none() {
            eprintln!("Warning: frontend/dist not found, serving API only");
        }

        let mut app = Router::new()
            .route("/rpc", post(handle_rpc_endpoint))
            .route("/chat", post(super::api::chat_api::handle_chat_endpoint))
            // ... 保留所有现有路由 ...
            .route(
                "/api/logs/stream",
                get(crate::server::api::log_api::handle_log_stream),
            );

        // 如果找到 frontend/dist，挂载静态文件服务
        if let Some(dist_path) = frontend_dist {
            let index_path = dist_path.join("index.html");
            app = app.fallback_service(
                tower_http::services::ServeDir::new(&dist_path)
                    .fallback(tower_http::services::ServeFile::new(&index_path)),
            );
        }

        let app = app
            .layer(cors_layer(self.state.config.clone()))
            .with_state(self.state.clone());

        // ... 剩余代码不变
```

> **注意：** `ServeDir` 作为 `fallback_service` 挂载在根路径。Axum 的匹配顺序是：`route` 优先于 `fallback_service`，因此 `/api/*`、`/chat`、`/rpc` 等 API 路由不受影响。当请求路径不匹配任何 API 路由时，才进入 `ServeDir` 查找文件。如果文件不存在，`ServeFile::new(index_path)` 返回 `index.html`，实现 SPA fallback。

- [ ] **Step 4: 编译验证**

Run: `cargo build -p fi-code-core`
Expected: 编译成功，无错误

- [ ] **Step 5: Commit**

```bash
git add crates/core/Cargo.toml crates/core/src/server/server.rs
git commit -m "feat(web-mode): add ServeDir static file service to Axum server"
```

---

## Task 3: CLI 参数与 Web 模式入口

**Files:**
- Modify: `crates/cli/Cargo.toml`
- Modify: `crates/cli/src/cli_args.rs`
- Modify: `crates/cli/src/entry.rs`

- [ ] **Step 1: 添加 open 依赖**

修改 `crates/cli/Cargo.toml`，在 `[dependencies]` 下添加：

```toml
open = "5"
```

- [ ] **Step 2: 新增 CLI 参数**

修改 `crates/cli/src/cli_args.rs`，在 `Args` struct 中 `workspace` 字段之后添加：

```rust
    /// Start web UI server (default port: 4040)
    #[arg(short = 'W', long = "web", value_name = "PORT", num_args = 0..=1)]
    pub web: Option<Option<u16>>,
```

- [ ] **Step 3: 在 entry.rs 中处理 web 参数**

在 `crates/cli/src/entry.rs` 的 `run()` 函数中，在 `match args.command { ... }` 之后、`#[cfg(debug_assertions)]` 日志配置之前，插入 web 模式处理逻辑：

```rust
    // -W / --web 模式
    if let Some(port_opt) = args.web {
        let port = port_opt.unwrap_or(4040);
        return start_web_mode(port).await;
    }
```

- [ ] **Step 4: 实现 start_web_mode 函数**

在 `crates/cli/src/entry.rs` 中 `EntryOutcome` 定义之后、`run()` 函数之前，添加新函数：

```rust
async fn start_web_mode(port: u16) -> anyhow::Result<EntryOutcome> {
    use anyhow::Context;
    use std::path::PathBuf;

    // 设置工作目录
    let workspace = dirs::home_dir().context("无法获取用户主目录")?;
    if !workspace.exists() {
        std::fs::create_dir_all(&workspace)
            .with_context(|| format!("无法创建工作目录: {:?}", workspace))?;
    }
    let workspace = workspace
        .canonicalize()
        .with_context(|| format!("无法解析工作目录: {:?}", workspace))?;
    fi_code_core::utils::workspace::set_workspace(workspace.clone());
    fi_code_core::skills::init_skills();

    let config = Arc::new(std::sync::RwLock::new(fi_code_core::config::Config::load()?));
    let _watcher = fi_code_core::config::config::spawn_watcher(Arc::clone(&config))?;

    // 初始化 MCP Manager
    {
        let cfg = config.read().map_err(|_| anyhow::anyhow!("配置锁中毒"))?;
        if let Some(mcp_config) = &cfg.mcp {
            match fi_code_core::mcp::manager::McpManager::from_config(mcp_config).await {
                Ok(manager) => {
                    fi_code_core::tools::set_mcp_manager(std::sync::Arc::new(manager));
                }
                Err(e) => {
                    eprintln!("Warning: MCP initialization failed: {}", e);
                }
            }
        }
    }

    let provider = Arc::new(fi_code_core::provider::Provider::new(Arc::clone(&config))?);
    fi_code_core::tools::set_task_provider(Arc::new(std::sync::RwLock::new((*provider).clone())));

    // 启动 Server
    let server = fi_code_core::server::Server::new(provider, config, Some(port));

    // 打开浏览器
    let url = format!("http://localhost:{}", port);
    if let Err(e) = open::that(&url) {
        eprintln!("Warning: failed to open browser: {}", e);
        println!("Please open {} manually", url);
    } else {
        println!("Opening browser at {} ...", url);
    }

    server.run().await;
    Ok(EntryOutcome::Completed)
}
```

- [ ] **Step 5: 编译验证**

Run: `cargo build -p fi-code-cli`
Expected: 编译成功

- [ ] **Step 6: Commit**

```bash
git add crates/cli/Cargo.toml crates/cli/src/cli_args.rs crates/cli/src/entry.rs
git commit -m "feat(web-mode): add -W/--web CLI arg and start_web_mode entry point"
```

---

## Task 4: 参数解析测试

**Files:**
- Modify: `tests/e2e/cli_e2e.rs`

- [ ] **Step 1: 添加 --web 参数存在性测试**

在 `tests/e2e/cli_e2e.rs` 的 `e2e_cli` module 中，现有测试之后添加：

```rust
    #[tokio::test]
    async fn test_cli_web_flag_in_help() {
        let output = run_cli(&["--help"]).await.expect("Failed to run CLI");
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("--web") || stdout.contains("-W"),
            "Help output should contain --web or -W flag"
        );
    }
```

- [ ] **Step 2: 运行 E2E 测试**

Run: `cargo test --test e2e_cli`
Expected: 所有测试通过（包括新增的帮助文档测试）

- [ ] **Step 3: Commit**

```bash
git add tests/e2e/cli_e2e.rs
git commit -m "test(web-mode): add E2E test for --web flag in help output"
```

---

## Task 5: 集成验证

**Files:**
- 无文件修改，纯验证步骤

- [ ] **Step 1: 完整编译**

Run: `cargo build`
Expected: 所有 crate 编译成功

- [ ] **Step 2: 运行全部 Rust 测试**

Run: `cargo test`
Expected: core 测试通过（忽略可能存在的网络测试偶发失败）

- [ ] **Step 3: 前端构建确认**

Run: `cd frontend && npm run build`
Expected: `frontend/dist/index.html` 存在

- [ ] **Step 4: 手动验证 web 模式启动**

Run: `./target/debug/fi-code-cli -W 9876`
Expected:
- 终端显示 "Opening browser at http://localhost:9876 ..."
- 浏览器自动打开（或在无图形环境时打印 URL）
- Server 监听 9876 端口
- 访问 `http://localhost:9876` 返回前端页面
- 访问 `http://localhost:9876/api/logs` 返回 JSON

- [ ] **Step 5: Commit 最终总结**

```bash
git commit --allow-empty -m "feat(web-mode): complete CLI web mode implementation

- Add -W/--web [PORT] flag to fi-code-cli (default 4040)
- Server serves frontend/dist as static files with SPA fallback
- Frontend auto-detects API base URL from window.location.origin
- Auto-opens system default browser on startup
- Compatible with existing Tauri Desktop mode"
```

---

## Self-Review

**1. Spec coverage:**
| Spec 需求 | 对应 Task |
|-----------|----------|
| CLI 参数 `-W` / `--web [PORT]` 默认 4040 | Task 3 |
| 自动打开浏览器 | Task 3 (Step 4) |
| 前端动态 base URL | Task 1 |
| Server 静态文件服务 | Task 2 |
| SPA fallback | Task 2 (Step 3) |
| 路径查找策略 | Task 2 (Step 2) |
| 依赖变更 (tower-http fs, open) | Task 2 Step 1, Task 3 Step 1 |
| 测试 | Task 4 |

**2. Placeholder scan:** 无 TBD/TODO，所有步骤含具体代码和命令。

**3. Type consistency：**
- `start_web_mode(port: u16)` 与 `Server::new(..., Some(port))` 类型一致
- `open::that(&url)` 接受 `&str`，`url` 为 `String`，通过 `&url` 传引用
- `ServeDir::new(&dist_path)` 接受 `impl AsRef<Path>`，`PathBuf` 符合要求

---

## 执行选项

Plan complete and saved to `docs/superpowers/plans/2026-05-18-cli-web-mode.md`.

**Two execution options:**

1. **Subagent-Driven (recommended)** - 我为每个 Task 分配一个 fresh subagent，确保每个改动独立、可 review

2. **Inline Execution** - 在当前会话中顺序执行所有 Task

**Which approach?**
