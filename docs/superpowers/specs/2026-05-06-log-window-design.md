# 日志窗口设计文档

> 日期：2026-05-06
> 状态：已评审，待实现

## 1. 需求概述

新增一个日志浮窗，用户按 `Ctrl+L` 后从底部弹出，占终端高度 60%，实时展示应用运行日志。日志来源包括服务端和 TUI 客户端自身。由于 TUI 与 Server 在同进程内运行，两者的日志均通过全局 `LogBroadcaster` 收集；TUI 通过独立 SSE 通道（`/api/logs/stream`）从 Server 拉取实时日志，并支持打开时通过 `GET /api/logs` 获取历史缓存。

需兼容三种运行模式：
- **TUI 模式**（默认）：日志通过 SSE 推送到 TUI 日志窗口，同时保留 stderr 输出
- **CLI / Interactive 模式**（`-i` / `-c`）：无 TUI、无 Server，日志只输出到 stderr
- **纯 Server 模式**（`--server`）：无 TUI，日志只输出到 stderr

## 2. 架构设计

### 2.1 日志数据流

```
┌─────────────────────────────────────────────────────────────┐
│                        TUI 模式                              │
│  ┌──────────┐      HTTP/SSE      ┌──────────────────────┐   │
│  │   TUI    │ ◄───────────────── │  Server (同进程)      │   │
│  │ LogWindow│  GET /api/logs     │  ┌───────────────┐   │   │
│  │          │  SSE /api/logs/stream│  │ LogBroadcaster│   │   │
│  └──────────┘                    │  │  + LogStore   │   │   │
│                                  │  └───────┬───────┘   │   │
│                                  │          │            │   │
│                                  │  ┌───────▼───────┐   │   │
│                                  │  │  log_info!    │   │   │
│                                  │  │  log_debug!   │   │   │
│                                  │  │  log_trace!   │   │   │
│                                  │  └───────────────┘   │   │
│                                  └──────────────────────┘   │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│                    CLI / Interactive 模式                    │
│  ┌──────────┐                                               │
│  │ log_info!│ ──► stderr (无 broadcaster，无 SSE)           │
│  └──────────┘                                               │
└─────────────────────────────────────────────────────────────┘
```

### 2.2 核心设计决策

- **全局可选广播器**：日志宏始终输出到 `stderr`，同时检查全局 `LogBroadcaster`；若已注册则额外广播。CLI 模式不注册，零开销。
- **独立 SSE 端点**：`/api/logs/stream` 专门用于日志推送，与 `/chat` SSE 完全隔离。
- **环形缓冲区**：服务端 `LogStore` 维护最近 1000 条日志，TUI 打开时先 `GET /api/logs` 拉取历史，再订阅 SSE 接收增量。

## 3. 核心数据结构

### 3.1 LogEntry

```rust
#[derive(Debug, Clone, Serialize)]
pub struct LogEntry {
    pub timestamp: String, // "HH:MM:SS" 或 ISO8601
    pub level: String,     // "INFO" | "DEBUG" | "TRACE" | "ERROR"
    pub module: String,
    pub message: String,
}
```

### 3.2 LogStore

```rust
pub struct LogStore {
    buffer: VecDeque<LogEntry>,
    capacity: usize, // 1000
}

impl LogStore {
    pub fn push(&mut self, entry: LogEntry);
    pub fn recent(&self, limit: usize) -> Vec<LogEntry>;
}
```

### 3.3 LogBroadcaster

```rust
pub struct LogBroadcaster {
    tx: tokio::sync::broadcast::Sender<LogEntry>,
    // 内部通过 mpsc::unbounded_channel 桥接同步发送与异步存储，
    // 确保 send() 可在非 async 上下文中被日志宏调用。
}

impl LogBroadcaster {
    /// 同步方法，供日志宏在非 async 上下文中调用。
    pub fn send(&self, level: &str, module: &str, message: String);
    
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<LogEntry>;
    pub async fn recent(&self, limit: usize) -> Vec<LogEntry>;
}
```

### 3.4 全局广播器注册

```rust
// src/utils/log.rs
use std::sync::{Arc, OnceLock};

static GLOBAL_LOG_BROADCASTER: OnceLock<Arc<LogBroadcaster>> = OnceLock::new();

pub fn set_global_log_broadcaster(b: Arc<LogBroadcaster>) {
    let _ = GLOBAL_LOG_BROADCASTER.set(b);
}

pub fn send_log(level: &str, module: &str, message: String) {
    // 始终输出到 stderr
    eprintln!("{} {}", log_prefix(level, module), message);
    // 如果 broadcaster 存在，额外广播
    if let Some(b) = GLOBAL_LOG_BROADCASTER.get() {
        b.send(level, module, message);
    }
}
```

## 4. 服务端变更

### 4.1 AppState 扩展

```rust
pub struct AppState {
    pub provider: Arc<RwLock<Provider>>,
    pub config: Arc<RwLock<Config>>,
    pub sessions: Arc<HttpSessionManager>,
    pub commands: Arc<CommandRegistry>,
    pub themes: Vec<crate::theme::ThemePreset>,
    pub current_theme: Arc<RwLock<String>>,
    pub log_broadcaster: Option<Arc<LogBroadcaster>>, // TUI 模式有值
}
```

### 4.2 HTTP 端点

- `GET /api/logs?limit={n}` → `ApiResponse<Vec<LogEntry>>`
  - 从 `LogStore` 读取最近 `n` 条（默认 200，最大 1000）
- `GET /api/logs/stream` → SSE `data: {...}`
  - 先发送 `recent(50)` 作为初始快照（可选）
  - 然后 `subscribe()` 进入实时广播循环

### 4.3 日志宏改造

`log_info!`、`log_debug!`、`log_trace!` 宏内部调用 `send_log()` 替代直接 `eprintln!`。

**兼容性保证**：即使 `GLOBAL_LOG_BROADCASTER` 未设置，`send_log()` 仍然会执行 `eprintln!`，因此 CLI 模式行为不变。

## 5. TUI 变更

### 5.1 LogWindow 组件

```rust
pub struct LogWindow {
    visible: bool,
    lines: Vec<LogLine>,
    scroll_offset: usize, // 0 = 底部最新
    auto_scroll: bool,
    disconnected: bool,
}

struct LogLine {
    timestamp: String,
    level: LogLevel,
    module: String,
    message: String,
}
```

**渲染**：
- 每行：`[HH:MM:SS] [LEVEL] [module] message`
- 颜色：`Info`=text_primary, `Debug`=text_secondary, `Trace`=text_muted, `Error`=error
- 断开时顶部显示红色横幅：`⚠ 日志连接已断开`

**交互**：
- `↑` / `↓` / 滚轮：上下滚动
- 收到新日志时如果已在底部，自动滚动到底部

### 5.2 布局调整

`LayoutManager` 新增字段：
```rust
pub struct LayoutManager {
    // ... 现有字段 ...
    pub log_window: bool,
}
```

`calculate()` 逻辑：
- 若 `log_window == true`：
  - `main` 区域高度 = 剩余高度的 40%
  - `log_window` 区域高度 = 剩余高度的 60%
  - 两者上下排列，宽度均占满终端
- 若 `log_window == false`：`main` 占满全部剩余高度（现有行为）

### 5.3 事件扩展

```rust
pub enum AppEvent {
    // ... 现有事件 ...
    ToggleLogWindow,
    SetLogHistory(Vec<LogLine>),
    AppendLog(LogLine),
    LogDisconnected,
}
```

### 5.4 快捷键

```rust
'l' => {
    // Ctrl+L
    self.handle_app_event(AppEvent::ToggleLogWindow).await;
}
```

`handle_esc_key()` 优先级：
1. 日志窗口打开 → `ToggleLogWindow`（关闭日志窗口）
2. 子菜单打开 → `CancelThemePreview`
3. 抽屉打开 → `close_drawers`
4. 其他 → 回到 Main

### 5.5 异步加载逻辑

```rust
fn handle_app_event(&mut self, event: AppEvent) {
    match event {
        AppEvent::ToggleLogWindow => {
            self.layout.log_window = !self.layout.log_window;
            if self.layout.log_window {
                self.spawn_load_logs();
                self.spawn_log_stream();
            }
        }
        AppEvent::SetLogHistory(lines) => {
            self.log_window.set_lines(lines);
        }
        AppEvent::AppendLog(line) => {
            self.log_window.append(line);
        }
        AppEvent::LogDisconnected => {
            self.log_window.set_disconnected(true);
        }
        // ...
    }
}

fn spawn_load_logs(&self) {
    let client = self.client.clone();
    let tx = self.event_tx.clone();
    tokio::spawn(async move {
        match client.get_logs(200).await {
            Ok(entries) => {
                let lines = entries.into_iter().map(|e| LogLine::from(e)).collect();
                let _ = tx.send(AppEvent::SetLogHistory(lines)).await;
            }
            Err(_) => {}
        }
    });
}

fn spawn_log_stream(&self) {
    let client = self.client.clone();
    let tx = self.event_tx.clone();
    tokio::spawn(async move {
        if let Err(_) = client.subscribe_logs(tx.clone()).await {
            let _ = tx.send(AppEvent::LogDisconnected).await;
        }
    });
}
```

### 5.6 TuiClient 扩展

```rust
impl TuiClient {
    pub async fn get_logs(&self, limit: usize) -> Result<Vec<LogEntry>>;
    
    /// 订阅日志 SSE 流，每收到一条日志发送 AppendLog 事件
    pub async fn subscribe_logs(&self, tx: mpsc::Sender<AppEvent>) -> Result<()>;
}
```

## 6. 数据流时序

```
用户启动 fi-code（TUI 模式）
  │
  ├─ entry::run_tui_mode()
  │   ├─ 创建 LogBroadcaster
  │   ├─ set_global_log_broadcaster(broadcaster.clone())
  │   ├─ 启动 Server（AppState.log_broadcaster = Some(...)）
  │   └─ 启动 TUI
  │
  ├─ Server 运行中
  │   └─ log_info!("agent started")
  │       ├─ eprintln!(...) → stderr
  │       ├─ broadcaster.send() → LogStore.push()
  │       └─ broadcast::Sender.send() → 无订阅者（TUI 尚未连接）
  │
  ├─ 用户按 Ctrl+L
  │   ├─ ToggleLogWindow → visible = true
  │   ├─ spawn_load_logs()
  │   │   └─ GET /api/logs?limit=200
  │   │       └─ SetLogHistory → 显示历史
  │   └─ spawn_log_stream()
  │       └─ SSE /api/logs/stream
  │           ├─ subscribe() 接收 broadcaster 消息
  │           └─ AppendLog → 实时追加
  │
  ├─ 用户按 Esc 或 Ctrl+L
  │   ├─ ToggleLogWindow → visible = false
  │   └─ SSE 连接断开
  │
  └─ 程序退出
      └─ broadcaster 随 Server 一起销毁
```

## 7. 边界情况

| 场景 | 处理 |
|------|------|
| CLI 模式（`-i` / `-c`） | `GLOBAL_LOG_BROADCASTER` 未设置，`send_log()` 只执行 `eprintln!`，无 SSE 开销 |
| SSE 连接断开 | TUI 显示红色断开提示，不自动重连（手动关闭再打开可恢复） |
| 终端 resize | `LayoutManager` 自动重新计算 60/40 分割 |
| 日志缓冲区满（1000 条） | 旧日志被覆盖，`GET /api/logs` 只返回最新 1000 条 |
| 日志涌入过快 | TUI 利用 `Tick`（80ms）批量刷新，避免每帧重绘；SSE channel 有缓冲 |
| 同时打开抽屉和日志窗口 | 左右抽屉正常显示，日志窗口占据底部 60% 的剩余宽度，互不影响 |
| 日志窗口打开时发送消息 | 聊天区域缩小到 40%，仍可正常对话，日志实时更新 |

## 8. 文件变更清单

| 文件 | 变更 |
|------|------|
| `src/utils/log.rs` | 改造日志宏：引入 `send_log()` 和全局 `GLOBAL_LOG_BROADCASTER`；保留 stderr 输出 |
| `src/utils/mod.rs` | 导出新的日志类型 |
| `src/server/log_api.rs` | **新建**：`LogEntry`、`LogStore`、`LogBroadcaster`；SSE 广播实现 |
| `src/server/server.rs` | `AppState` 增加 `log_broadcaster`；注册 `/api/logs` 和 `/api/logs/stream` 路由；TUI 模式下初始化 broadcaster |
| `src/tui/client.rs` | 新增 `get_logs()` 和 `subscribe_logs()` |
| `src/tui/event.rs` | 新增 `ToggleLogWindow`、`SetLogHistory`、`AppendLog`、`LogDisconnected` |
| `src/tui/layout.rs` | `LayoutManager` 增加 `log_window` 状态；`calculate()` 支持 60/40 分割 |
| `src/tui/components/log_window.rs` | **新建**：`LogWindow` 组件 |
| `src/tui/components/mod.rs` | 导出 `LogWindow` |
| `src/tui/app.rs` | 初始化 `LogWindow`；`Ctrl+L` 快捷键；`Esc` 优先级；处理日志事件 |
| `src/entry.rs` | `run_tui_mode()` 中创建并注册 `LogBroadcaster` |
| `docs/superpowers/specs/2026-05-06-log-window-design.md` | **新建**：本文档 |
