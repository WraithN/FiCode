# 重构记录：CLI 模块提取

**处理时间**：2026-05-14 20:30
**模块**：`crates/cli`、`crates/core`
**相关 Commit**：(待填充)

---

## 重构动机

`crates/core/src/entry.rs` 和 `crates/core/src/utils/cli.rs` 是纯粹的 CLI 入口代码，职责包括：
- 命令行参数解析（`clap`）
- CLI 子命令路由（`-i` 交互模式、`-c` 单命令模式、`-s` session 列表、`server` 子命令等）
- REPL 交互循环
- 会话选择/创建

这些代码**只有 `fi-code-cli` 二进制在使用**（`fi-code-server` 和 `fi-code-tui` 都有独立的入口）。将其保留在 `core` 中导致：
1. `core` 必须依赖 `clap`（仅用于 CLI 参数解析）
2. `core` 的 API 表面混入 CLI 专属逻辑，职责不清晰
3. `fi-code-server` 和 `fi-code-tui` 编译时被迫间接依赖不必要的 CLI 代码

---

## 具体改动

### 1. 文件迁移

| 原位置 | 新位置 | 说明 |
|--------|--------|------|
| `crates/core/src/entry.rs` | `crates/cli/src/entry.rs` | CLI 主入口，所有 `crate::` 导入改为 `fi_code_core::` |
| `crates/core/src/utils/cli.rs` | `crates/cli/src/cli_args.rs` | `clap` 参数定义（`Args`、`Commands`） |

### 2. 导入更新

`crates/cli/src/entry.rs` 中的导入全部从 `crate::` 改为 `fi_code_core::`：
- `crate::agent::{agent_loop, LoopState}` → `fi_code_core::agent::{agent_loop, LoopState}`
- `crate::commands::slash::...` → `fi_code_core::commands::slash::...`
- `crate::config::Config` → `fi_code_core::config::Config`
- `crate::mcp::manager::McpManager` → `fi_code_core::mcp::manager::McpManager`
- `crate::provider::Provider` → `fi_code_core::provider::Provider`
- `crate::session::...` → `fi_code_core::session::...`
- `crate::tools::...` → `fi_code_core::tools::...`
- `crate::utils::workspace::set_workspace` → `fi_code_core::utils::workspace::set_workspace`
- `crate::utils::cli::{Args, Commands}` → `crate::cli_args::{Args, Commands}`（本地模块）
- `crate::server::Server` → `fi_code_core::server::Server`
- `crate::skills::...` → `fi_code_core::skills::...`
- `crate::{log_debug, log_info}` → `fi_code_core::{log_debug, log_info}`

### 3. 模块导出调整

- `crates/core/src/lib.rs`：**移除** `pub mod entry;`
- `crates/core/src/utils/mod.rs`：**移除** `pub mod cli;`
- `crates/cli/src/main.rs`：改为引用本地 `mod entry;` 和 `mod cli_args;`

### 4. 依赖调整

**`crates/core/Cargo.toml`：**
- **移除** `clap = { version = "4", features = ["derive"] }`（唯一用途是 `entry.rs` + `utils/cli.rs`）
- 保留 `colored`、`directories`、`dirs`（仍被 `commands/slash.rs`、`tools/mod.rs`、`config/config.rs`、`skills/scanner.rs`、`utils/workspace.rs` 使用）

**`crates/cli/Cargo.toml`：**
- **新增** `clap = { version = "4", features = ["derive"] }`
- **新增** `colored = "3.1.1"`（`entry.rs` 使用 `Colorize`）
- **新增** `directories = "5.0"`（`entry.rs` 计算 config_dir）
- **新增** `dirs = "5.0"`（`entry.rs` 获取 home_dir）

---

## 预期收益

1. **职责清晰**：`core` 专注于业务逻辑（Agent、Server、Session、Tools 等），`cli` 专注于命令行入口
2. **编译优化**：`core` 移除 `clap` 依赖后，编译 `fi-code-server` 和 `fi-code-tui` 时不再需要编译 `clap`
3. **可维护性**：CLI 专属逻辑（参数解析、REPL 循环）集中在 `cli` crate，与 core 解耦
4. **为 future 拆分做准备**：如果后续要将 `server` 也从 core 提取，`entry.rs` 中 `server` 子命令的调用方式可以独立调整

---

## 验证

- `cargo build --workspace`：编译成功，无错误
- `cargo test --workspace`：全部 249 个测试通过，0 失败
