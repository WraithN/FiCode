# 重构记录：TurnState 状态封装

**处理时间**：2026-05-14 20:45
**模块**：`crates/core/src/agent`
**相关 Commit**：(待填充)

---

## 重构动机

`run_one_turn`（无论是旧版 `agent.rs` 中的全局函数，还是新版 `runner.rs` 中的 `AgentRunner` 方法）内部存在大量散乱的状态变量：

- `content_blocks` — 聚合 LLM 流式输出
- `finish_reason` — LLM 停止原因
- `turn_usage` — 本轮 Token 使用量
- `session_id` — 会话 ID
- `wave_marker` — WaveMarker 元信息
- `token_baseline` — WaveMarker delta 计算基线
- `assistant_idx` — Assistant 消息索引

这些变量贯穿整轮对话的生命周期，在函数内各处被读取和修改，导致：
1. 代码可读性差，难以快速理解一轮对话的数据流
2. `process_chunk` 和 `update_wave_marker` 等辅助函数需要传递大量参数
3. 两个版本的 `run_one_turn` 存在大量重复的状态管理逻辑

---

## 具体改动

### 1. 新增 `TurnState` 结构体（`agent.rs`）

```rust
pub struct TurnState {
    pub content_blocks: Vec<Part>,
    pub finish_reason: Option<FinishReason>,
    pub turn_usage: TokenUsage,
    pub session_id: String,
    pub wave_marker: Part,
    pub token_baseline: TokenUsage,
    pub assistant_idx: usize,
}
```

### 2. 将独立函数提取为 `TurnState` 方法

| 原独立函数 | 新方法 | 说明 |
|-----------|--------|------|
| `process_chunk(chunk, &mut content_blocks, &mut finish_reason, &mut turn_usage)` | `turn.process_chunk(chunk)` | 方法内部直接操作状态字段，无需传递多个 `&mut` 参数 |
| `update_wave_marker(messages, idx, total, current_usage, baseline)` | `turn.update_wave_marker(messages, total, current_usage)` | `idx` 和 `baseline` 从 `self.assistant_idx` 和 `self.token_baseline` 获取 |

### 3. 新增辅助方法

- `TurnState::new(session_id, wave_step, token_baseline)` — 统一初始化
- `TurnState::append_assistant_message(messages)` — 组装并追加 Assistant 消息，记录 `assistant_idx`
- `TurnState::needs_mcp_two_step()` — 判断是否需要 MCP 两步发现
- `TurnState::accumulate_token_usage(state)` — 累加本轮 Token 到 LoopState

### 4. 重构 `agent.rs` 中的 `run_one_turn`

使用 `TurnState` 后，原 350+ 行的函数核心逻辑简化为：

```rust
let mut turn = TurnState::new(session_id, state.turn_count as u32 + 1, state.token_usage.clone());
// ... 发送 WaveMarker SSE ...
client.stream_message(..., &mut |chunk| {
    // ... 转发 SSE ...
    turn.process_chunk(chunk);
}).await?;
// ... 日志 ...
turn.append_assistant_message(&mut state.messages);
turn.accumulate_token_usage(state);
// ... 分支处理 ...
```

### 5. 重构 `runner.rs` 中的 `AgentRunner::run_one_turn`

同样使用 `TurnState`，移除本地的 `process_chunk` 关联函数和 `update_wave_marker_runner` 独立函数。

### 6. 删除的代码

- `agent.rs`：独立的 `process_chunk` 函数、独立的 `update_wave_marker` 函数
- `runner.rs`：`AgentRunner::process_chunk` 关联函数、`update_wave_marker_runner` 独立函数

---

## 预期收益

1. **状态内聚**：所有单轮状态集中在 `TurnState` 中，不再散落在函数各处
2. **参数简化**：`process_chunk` 从 4 个参数减为 1 个（`chunk`），`update_wave_marker` 从 5 个参数减为 3 个
3. **消除重复**：两个版本的 `run_one_turn` 共享同一套状态管理逻辑
4. **可读性提升**：`run_one_turn` 的主流程更清晰，关注点分离更好

---

## 验证

- `cargo build --workspace`：编译成功，0 错误，0 警告
- `cargo test --workspace`：全部 249 个测试通过，0 失败
