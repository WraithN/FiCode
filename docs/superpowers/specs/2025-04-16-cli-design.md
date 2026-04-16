# CLI 命令行选项支持设计文档

日期：2025-04-16

---

## 1. 背景与目标

当前 `shun-code` 启动后直接进入交互式 REPL，缺乏命令行参数支持。本设计新增一组 CLI 选项，使用户能够：

- 查看版本与帮助信息
- 控制调试日志输出
- 直接进入交互式模式（显式声明）
- 查看会话历史（列表或指定会话详情）
- 执行单条命令后退出（非交互式使用）

---

## 2. CLI 选项定义

引入 `clap` crate，采用 derive 宏定义参数结构。

```rust
#[derive(Parser, Debug)]
#[command(name = "shun-code", version = env!("CARGO_PKG_VERSION"))]
pub struct Args {
    /// Enable debug logging (debug|info, default: info)
    #[arg(short = 'l', long = "log", value_name = "LEVEL", default_value = "info")]
    pub log_level: String,

    /// Enter interactive REPL mode
    #[arg(short = 'i', long = "interactive")]
    pub interactive: bool,

    /// Print session information and exit
    ///   -s         -> 列出所有会话摘要
    ///   -s <id>    -> 打印指定会话的消息历史（id 支持完整字符串或前 4 位前缀匹配）
    ///   -s last    -> 打印最近一个会话
    ///   -s last-1  -> 打印倒数第二个会话
    #[arg(short = 's', long = "session", value_name = "SESSION", num_args = 0..=1)]
    pub session: Option<Option<String>>,

    /// Execute a single command and exit
    #[arg(short = 'c', long = "command", value_name = "MESSAGE")]
    pub command: Option<String>,
}
```

> `-v/--version` 和 `-h/--help` 由 `clap` 内置的 `ArgAction::Version` / `ArgAction::Help` 自动处理，无需额外代码。

---

## 3. 日志系统

为了避免引入重型日志框架，采用轻量级全局开关 + 宏的方案。

### 3.1 全局开关

在 `src/log.rs` 中定义：

```rust
use std::sync::atomic::{AtomicBool, Ordering};

static DEBUG_ENABLED: AtomicBool = AtomicBool::new(false);

pub fn set_debug(enabled: bool) {
    DEBUG_ENABLED.store(enabled, Ordering::Relaxed);
}

pub fn is_debug() -> bool {
    DEBUG_ENABLED.load(Ordering::Relaxed)
}
```

### 3.2 宏定义

```rust
#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {
        if $crate::log::is_debug() {
            eprintln!("[debug] {}", format!($($arg)*));
        }
    };
}
```

### 3.3 初始化行为

`main.rs` 在 `Args::parse()` 后立即调用：

```rust
set_debug(args.log_level.eq_ignore_ascii_case("debug"));
```

- `info`（默认）：`log_debug!` 不输出
- `debug`：`log_debug! 输出到 stderr`

现有代码中所有 `eprintln!("[debug] ...")` 统一替换为 `log_debug!(...)`。

---

## 4. 执行流程与优先级

```
parse args
    |
    v
set_debug(log_level)
    |
    v
args.session ?      -> print_session() -> exit(0)
    |
    v
args.command ?      -> run_single_command() -> exit(0)
    |
    v
run_interactive()   -> REPL 主循环
```

优先级：**`-s` > `-c` > 默认交互式**。多个选项同时传入时，按此顺序仅执行第一个匹配的分支。

---

## 5. `-s --session` 详细行为

`SessionManager` 新增辅助方法 `find_session_index(&self, selector: &str) -> Result<Session>`。

| 输入形式 | 行为 |
|---------|------|
| `-s`（无参数） | 打印所有会话的摘要列表（同 `list_sessions` 格式），按 `updated_at` 降序 |
| `-s last` | 加载并打印最近更新的会话的完整消息历史 |
| `-s last-1` | 加载并打印倒数第二个会话的完整消息历史 |
| `-s <id>` | 先尝试精确匹配 `session_id`；若失败，尝试前缀匹配前 4 位（或更长前缀）；唯一匹配时加载打印，多个匹配时报错 |

### 消息打印格式

每条消息输出：

```
[User] hello
[Assistant] pong
[Assistant -> tool_use: calculator] {"a": 23, "b": 45}
[User -> tool_result: calculator] 68
```

文本内容直接打印；`ToolUse` / `ToolResult` 打印摘要行；`Image` / `Reasoning` 以占位符表示。

---

## 6. 代码结构变更

### 新增文件
- `src/cli.rs` — CLI 参数定义（`Args` struct）
- `src/log.rs` — 全局 debug 开关与 `log_debug!` 宏

### 修改文件
- `Cargo.toml` — 添加 `clap = { version = "4", features = ["derive"] }` 依赖
- `src/main.rs` — 按优先级调度各分支；原有 REPL 逻辑提取为 `run_interactive`
- `src/agent/agent.rs` / `src/tools/mod.rs` / `src/provider/client/*.rs` — 替换 `eprintln!("[debug] ...")` 为 `log_debug!(...)`
- `src/session/session.rs` — 新增 `find_session_index`、`print_session_messages` 等辅助方法

---

## 7. 测试策略

1. **CLI 解析测试**：在 `src/cli.rs` 中使用 `clap::Parser::try_parse_from` 验证各选项组合能正确解析。
2. **日志宏测试**：验证 `set_debug(true)` 后 `log_debug!` 输出到 stderr，`set_debug(false)` 时不输出。
3. **Session 查找测试**：在 `session.rs` 中增加 `find_session_index` 的单元测试，覆盖 `last`、`last-1`、精确匹配、前缀匹配、歧义匹配场景。
4. **单命令模式集成测试**：通过 `cargo run -- -c "..."` 手动验证执行后正确退出（测试环境可结合已有 provider 单测逻辑）。

---

## 8. 边界情况

- 无前缀匹配到任何会话时，返回错误并打印提示。
- `-s last-1` 但会话总数不足 2 个时，返回越界错误。
- `--log` 传入非 `debug`/`info` 的值时，按 `info` 处理（静默降级）。
- `-c` 传入空字符串时，视为空输入，直接退出不调用 LLM。
