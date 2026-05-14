# Bug 记录清单

> 本文件记录 fi-code 项目中已修复的 Bug。每解决一个 Bug 后追加一条记录，便于追溯历史问题和积累排查经验。
>
> 格式：日期 | 模块 | 现象 | 根因 | 修复方案 | 相关 Commit

---

## 2025-05-14 | TUI Chat 自动滚动 | 新消息到达后窗口不自动跟随到底部

**现象**：当 Chat 区域有新消息或 SSE 流式内容到达时，窗口不会自动滚动显示最新内容，用户需要手动按 PageDown 或滚轮滚动才能看到新内容。

**根因**：`Chat` 的 `scroll_offset` 是从**内容顶部**开始计数的，`scroll_offset = max_scroll_offset` 才表示底部。当用户已在底部时，新内容追加使 `max_scroll_offset` 增大，但 `scroll_offset` 仍保持旧的值不变，导致用户看到的是"旧的底部"，新内容被截断在下方不可见。

**修复方案**：
1. 新增 `auto_scroll: bool` 标志（默认 `true`）
2. `draw` 时若 `auto_scroll` 开启，直接使用当前计算出的 `max_scroll_offset` 作为视口起点，始终锁定底部
3. 用户手动向上滚动（`PageUp` / 滚轮向上）时暂停 `auto_scroll`
4. 用户回到底部时恢复 `auto_scroll`
5. `clear_messages` 时重置 `auto_scroll`
6. 补充 7 个单元测试覆盖各场景

**相关 Commit**: `900d53d`

---

## 2025-05-14 | Tools Task Plan | 子任务执行 panic：timers are disabled

**现象**：执行 Task Plan（任务拆分）时，子任务执行阶段 panic，错误信息为 `A Tokio 1.x context was found, but timers are disabled`。

**根因**：`crates/core/src/tools/task/tool.rs` 中创建了一个 `tokio::runtime::Builder::new_current_thread()` 运行时来执行子任务，但**没有调用 `.enable_time()`** 启用计时器。而子任务内部调用 AI 客户端时，若遇到网络错误需要重试，`base_client.rs` 中的 `send_with_retry` 会使用 `tokio::time::sleep(backoff).await` 进行退避，导致 panic。

**修复方案**：在运行时构建器上添加 `.enable_time()`。

```rust
let rt = tokio::runtime::Builder::new_current_thread()
    .enable_time()
    .build()
    .unwrap();
```

**相关 Commit**: `5d96cb6`

---
