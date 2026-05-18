# CLI Web 模式设计文档

> 让 fi-code-cli 支持一键启动 Web 界面，复用 Desktop 前端页面。

---

## 1. 需求概述

在 CLI 中新增 `-W` / `--web [PORT]` 参数，启动后：
1. 启动 HTTP Server（API + SSE），监听指定端口
2. 从 `frontend/dist` 目录提供静态文件服务
3. 自动打开系统默认浏览器访问 `http://localhost:PORT`
4. 前端通过 `window.location.origin` 动态连接后端，支持任意端口

---

## 2. 架构设计

```
┌─────────────────────────────────────────────┐
│  fi-code-cli -W 8080                         │
│                                              │
│  ┌─────────────┐    ┌──────────────────┐    │
│  │  Axum Router │───▶│ ServeDir         │    │
│  │             │    │ (frontend/dist)  │    │
│  │  /api/*     │    │ fallback→index.html│  │
│  │  /chat      │    └──────────────────┘    │
│  │  /rpc       │                             │
│  └─────────────┘    ┌──────────────────┐    │
│                     │ open::that()     │    │
│                     │ (打开浏览器)      │    │
│                     └──────────────────┘    │
└─────────────────────────────────────────────┘
```

### 路由优先级
1. `/api/*`、`/chat`、`/rpc` 等 API 路由优先匹配
2. 根路径 `/` 及未匹配路径由 `ServeDir` 处理
3. SPA fallback：静态文件不存在时返回 `frontend/dist/index.html`

---

## 3. CLI 参数设计

```rust
/// Start web UI server (default port: 4040)
#[arg(short = 'W', long = "web", value_name = "PORT", num_args = 0..=1)]
pub web: Option<Option<u16>>,
```

### 使用方式
| 命令 | 行为 |
|------|------|
| `fi-code -W` | 启动 web 模式，端口 4040，自动打开浏览器 |
| `fi-code -W 8080` | 启动 web 模式，端口 8080，自动打开浏览器 |
| `fi-code --web` | 同上，端口 4040 |
| `fi-code --web 3000` | 同上，端口 3000 |

---

## 4. 启动流程

```rust
pub async fn run() -> Result<EntryOutcome> {
    let args = Args::parse();

    // 子命令优先
    match args.command { ... }

    // -W / --web 模式
    if let Some(port_opt) = args.web {
        let port = port_opt.unwrap_or(4040);
        return start_web_mode(port).await;
    }

    // 其他模式...
}

async fn start_web_mode(port: u16) -> Result<EntryOutcome> {
    // 1. 初始化与 interactive 模式相同的依赖
    let workspace = ...;
    let config = Arc::new(RwLock::new(Config::load()?));
    let provider = Arc::new(Provider::new(Arc::clone(&config))?);
    let session_manager = SessionManager::new(...);

    // 2. 初始化 MCP、Skills 等
    ...

    // 3. 启动 Server（复用 fi_code_core::server::Server）
    let server = fi_code_core::server::Server::new(provider, config, Some(port));

    // 4. 打开浏览器
    let url = format!("http://localhost:{}", port);
    if let Err(e) = open::that(&url) {
        eprintln!("Warning: failed to open browser: {}", e);
        println!("Please open {} manually", url);
    }

    // 5. 阻塞运行 Server
    server.run().await;
    Ok(EntryOutcome::Completed)
}
```

---

## 5. Server 静态文件服务

### 5.1 路由配置

在 `crates/core/src/server/server.rs` 中，Router 增加静态文件服务：

```rust
use tower_http::services::ServeDir;

let app = Router::new()
    .route("/api/files", get(file_api::file_tree))
    .route("/api/files/content", get(file_api::file_content))
    .route("/api/logs", get(log_api::handle_list_logs))
    .route("/api/logs/stream", get(log_api::handle_log_stream))
    // ... 其他 API 路由
    .route("/chat", post(chat_api::handle_chat))
    .route("/rpc", post(rpc_handler))
    .fallback_service(
        ServeDir::new("frontend/dist")
            .fallback(ServeFile::new("frontend/dist/index.html"))
    )
    .layer(...)
    .with_state(app_state);
```

### 5.2 SPA Fallback

React 前端使用 client-side routing（`react-router-dom` 或类似方案）。当用户直接访问 `/chat/session-xxx` 时，Axum 找不到对应文件，需要返回 `index.html`，由前端路由接管。

使用 `ServeDir::fallback` 实现：
```rust
ServeDir::new(frontend_dist_path)
    .fallback(ServeFile::new(index_html_path))
```

### 5.3 路径配置

静态文件根目录通过以下优先级确定：
1. 运行时检测：从可执行文件所在目录向上查找 `frontend/dist`
2. 若找不到，打印警告，Server 仅提供 API 服务

```rust
fn find_frontend_dist() -> Option<PathBuf> {
    let exe_dir = std::env::current_exe()
        .ok()?
        .parent()?
        .to_path_buf();
    
    // 尝试相对路径
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

---

## 6. 前端动态 Base URL

### 6.1 当前问题

`frontend/src/services/apiClient.ts` 中：
```typescript
constructor(baseUrl: string = 'http://localhost:4040') {
```

如果用户用 `-W 8080` 启动，前端会尝试连接 4040 端口，导致失败。

### 6.2 修复方案

将默认 base URL 改为从 `window.location.origin` 推断：

```typescript
export class ApiClient {
  private baseUrl: string;

  constructor(baseUrl?: string) {
    this.baseUrl = (baseUrl ?? window.location.origin).replace(/\/$/, '');
  }
  // ...
}
```

这样无论 Server 监听哪个端口，前端都会自动连接到正确的地址。

### 6.3 Desktop 兼容性

Desktop 应用（Tauri）的前端通过 `127.0.0.1:4040` 访问 Sidecar。Tauri 中 `window.location.origin` 是 `tauri://localhost`（或类似协议），不适用于 API 调用。

因此需要区分环境：
- **Web 模式**：`window.location.origin`
- **Desktop 模式**：保持 `http://127.0.0.1:4040`

检测方式：检查 `window.__TAURI_INTERNALS__` 或 `window.__TAURI__` 是否存在。

```typescript
function detectBaseUrl(): string {
  // Tauri 环境
  if ((window as any).__TAURI_INTERNALS__) {
    return 'http://127.0.0.1:4040';
  }
  // 浏览器环境（Web 模式）
  return window.location.origin;
}
```

---

## 7. 依赖变更

### 7.1 `crates/core/Cargo.toml`

```toml
tower-http = { version = "0.5", features = ["cors", "fs"] }
```

增加 `fs` feature 以启用 `ServeDir` 和 `ServeFile`。

### 7.2 `crates/cli/Cargo.toml`

```toml
[dependencies]
open = "5"
```

`open` crate 提供跨平台打开浏览器的能力（支持 Windows、macOS、Linux）。

---

## 8. 错误处理

| 场景 | 行为 |
|------|------|
| `frontend/dist` 不存在 | 打印警告：`Warning: frontend/dist not found, serving API only`，Server 正常启动 |
| 端口被占用 | 打印错误并退出，提示用户更换端口 |
| 打开浏览器失败 | 打印警告和访问 URL，用户可手动打开 |
| 无图形环境（SSH/容器） | 同“打开浏览器失败”处理 |

---

## 9. 测试策略

### 9.1 单元测试
- `cli_args.rs`：验证 `--web`、`--web 8080`、`-W`、`-W 8080` 的解析结果
- `find_frontend_dist()`：验证路径查找逻辑

### 9.2 E2E 测试
- 启动 `fi-code-cli -W <随机端口>`，验证：
  1. Server 正常启动并监听指定端口
  2. `GET /` 返回 `index.html`
  3. `GET /api/logs` 返回 JSON
  4. `GET /chat`（POST）正常工作

---

## 10. 安全注意事项

- `ServeDir` 严格限制在 `frontend/dist` 目录内，防止路径遍历攻击
- 不暴露 `.git`、`.env` 等敏感文件（`frontend/dist` 是构建产物，通常不包含这些）
- API 路由优先于静态文件路由，避免恶意文件覆盖 API 端点
