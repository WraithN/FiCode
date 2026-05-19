# Context Compression Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 fi-code Agent 对话系统引入上下文压缩机制，包括 Token 估算、工具结果动态压缩、增量式历史摘要。

**Architecture:** 新增 `agent/compression.rs` 模块，提供 Token 估算、阈值检测、压缩范围计算、增量压缩执行。`LoopState` 扩展 `compression_summary` 字段保存内存中的增量摘要。`run_one_turn` 在每轮开头检查阈值并触发压缩。工具结果在 `execute_tool_calls` 中根据压缩状态动态截断。

**Tech Stack:** Rust, tokio, anyhow, existing AgentRunner/AIClient/LoopState/Message/Part infrastructure

---

## File Structure

| File | Action | Responsibility |
|------|--------|---------------|
| `crates/core/src/agent/compression.rs` | Create | Token 估算、阈值检测、压缩范围计算、tool 配对保护、增量压缩、subagent 调用、build_llm_messages |
| `crates/core/src/agent/agent.rs` | Modify | LoopState 添加 `compression_summary`；run_one_turn 集成压缩检查 |
| `crates/core/src/agent/mod.rs` | Modify | 导出 compression 模块 |
| `crates/core/src/tools/mod.rs` | Modify | execute_tool_calls 中动态压缩 tool result |
| `crates/core/src/provider/provider.rs` | Modify | 新增 `context_limit()` 方法，暴露模型上下文大小 |
| `crates/shared/src/constants.rs` | Modify | 添加上下文压缩相关常量 |

---

## Task 1: 添加上下文压缩常量

**Files:**
- Modify: `crates/shared/src/constants.rs`

- [ ] **Step 1: 在 constants.rs 末尾添加上下文压缩常量**

```rust
// =============================================================================
// 上下文压缩相关常量
// =============================================================================

/// 默认上下文限制（token 数）：128K
pub const DEFAULT_CONTEXT_LIMIT: u32 = 128_000;

/// 压缩触发阈值：上下文使用率达到 85% 时触发
pub const COMPRESSION_THRESHOLD: f64 = 0.85;

/// 工具结果正常压缩阈值（字符数）：超过此值进行头尾截断
pub const TOOL_RESULT_COMPRESS_THRESHOLD_NORMAL: usize = 8_000;

/// 工具结果激进压缩阈值（字符数）：上下文紧张时使用更激进的阈值
pub const TOOL_RESULT_COMPRESS_THRESHOLD_AGGRESSIVE: usize = 3_000;

/// 工具结果压缩后保留的头部字符数
pub const TOOL_RESULT_COMPRESS_HEAD: usize = 1_000;

/// 工具结果压缩后保留的尾部字符数
pub const TOOL_RESULT_COMPRESS_TAIL: usize = 2_000;
```

- [ ] **Step 2: 运行编译检查**

Run: `cargo check -p fi-code-shared`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/shared/src/constants.rs
git commit -m "feat: add context compression constants"
```

---

## Task 2: Provider 暴露上下文限制

**Files:**
- Modify: `crates/core/src/provider/provider.rs`

- [ ] **Step 1: 在 Provider impl 中添加 context_limit 方法**

在 `crates/core/src/provider/provider.rs` 中 `impl Provider` 块内，`model_name()` 方法之后添加：

```rust
    /// 获取当前模型的上下文限制（token 数）。
    ///
    /// 从 Config 中查找当前模型对应的 `limit.context`，
    /// 若未配置则返回 `DEFAULT_CONTEXT_LIMIT`（128K）。
    pub fn context_limit(&self, config: &crate::config::Config) -> u32 {
        let model_name = self.model_name().unwrap_or("unknown");

        for (_, provider_cfg) in &config.provider {
            if let Some(model_cfg) = provider_cfg.models.get(model_name) {
                if let Some(ref limit) = model_cfg.limit {
                    return limit.context;
                }
            }
        }

        DEFAULT_CONTEXT_LIMIT
    }
```

- [ ] **Step 2: 编译检查**

Run: `cargo check -p fi-code-core`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/provider/provider.rs
git commit -m "feat: add Provider::context_limit() to expose model context window"
```

---

## Task 3: 创建 compression.rs 模块 — Token 估算与全局 Context Limit

**Files:**
- Create: `crates/core/src/agent/compression.rs`
- Modify: `crates/core/src/agent/mod.rs`

- [ ] **Step 1: 创建 compression.rs 文件**

```rust
// MIT License
// Copyright (c) 2025 fi-code contributors
// ... (完整 MIT 许可证头)

// =============================================================================
// 上下文压缩模块
// =============================================================================
// 本模块负责：
// 1. Token 估算（当 LLM 不返回 usage 时使用）
// 2. 压缩阈值检测
// 3. 工具结果动态压缩
// 4. 历史消息范围计算（含 tool_use/tool_result 配对保护）
// 5. 增量压缩执行（通过 subagent summarize）
// 6. 构建供 LLM 使用的压缩消息视图

use std::collections::HashSet;
use std::sync::atomic::{AtomicU32, Ordering};

use anyhow::Result;

use fi_code_shared::constants::*;
use fi_code_shared::dto::{Message, Part, Role, TokenUsage};

use crate::agent::{AgentRunner, AgentType, LoopState};
use crate::agent::profile::{AgentProfile, ToolFilter};
use crate::provider::base_client::AIClient;

// ---------------------------------------------------------------------------
// 全局上下文限制（由调用方根据配置设置）
// ---------------------------------------------------------------------------

static CONTEXT_LIMIT: AtomicU32 = AtomicU32::new(DEFAULT_CONTEXT_LIMIT);

pub fn set_context_limit(limit: u32) {
    CONTEXT_LIMIT.store(limit, Ordering::Relaxed);
}

pub fn get_context_limit() -> u32 {
    CONTEXT_LIMIT.load(Ordering::Relaxed)
}

// ---------------------------------------------------------------------------
// Token 估算
// ---------------------------------------------------------------------------

const TOKEN_WEIGHT_ASCII: f64 = 0.25;
const TOKEN_WEIGHT_NON_ASCII: f64 = 0.67;

/// 估算文本的 token 数。
pub fn estimate_tokens(text: &str) -> u32 {
    text.chars()
        .map(|c| if c.is_ascii() { TOKEN_WEIGHT_ASCII } else { TOKEN_WEIGHT_NON_ASCII })
        .sum::<f64>()
        .ceil() as u32
}

/// 估算单条消息的 token 数。
pub fn estimate_message_tokens(msg: &Message) -> u32 {
    msg.parts.iter().map(|part| match part {
        Part::Text { text } => estimate_tokens(text),
        Part::ToolResult { content, .. } => estimate_tokens(content),
        Part::ToolError { content, .. } => estimate_tokens(content),
        _ => 20,
    }).sum()
}

/// 估算消息列表的总 token 数。
pub fn estimate_total_tokens(messages: &[Message]) -> u32 {
    messages.iter().map(estimate_message_tokens).sum()
}

// ---------------------------------------------------------------------------
// 阈值检测
// ---------------------------------------------------------------------------

/// 判断是否应该触发上下文压缩。
pub fn should_compress(messages: &[Message]) -> bool {
    let limit = get_context_limit();
    let threshold = (limit as f64 * COMPRESSION_THRESHOLD) as u32;
    let estimated = estimate_total_tokens(messages);
    estimated >= threshold
}

// ---------------------------------------------------------------------------
// 工具结果动态压缩
// ---------------------------------------------------------------------------

/// 压缩工具结果内容。
pub fn compress_tool_result(content: &str, is_aggressive: bool) -> String {
    let threshold = if is_aggressive {
        TOOL_RESULT_COMPRESS_THRESHOLD_AGGRESSIVE
    } else {
        TOOL_RESULT_COMPRESS_THRESHOLD_NORMAL
    };

    if content.len() <= threshold {
        return content.to_string();
    }

    let head_end = content
        .char_indices()
        .nth(TOOL_RESULT_COMPRESS_HEAD)
        .map(|(i, _)| i)
        .unwrap_or(content.len());
    let tail_start = content.len().saturating_sub(TOOL_RESULT_COMPRESS_TAIL);
    let truncated = content.len() - head_end - (content.len() - tail_start);

    format!(
        "{}\n\n... [{} chars truncated] ...\n\n{}",
        &content[..head_end],
        truncated,
        &content[tail_start..]
    )
}
```

- [ ] **Step 2: 在 agent/mod.rs 中导出 compression 模块**

在 `crates/core/src/agent/mod.rs` 中，在现有 `pub use` 行附近添加：

```rust
pub mod compression;
```

- [ ] **Step 3: 编译检查**

Run: `cargo check -p fi-code-core`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/agent/compression.rs crates/core/src/agent/mod.rs
git commit -m "feat: add agent/compression.rs with token estimation and tool result compression"
```

---

## Task 4: compression.rs — 压缩范围计算（含 tool 配对保护）

**Files:**
- Modify: `crates/core/src/agent/compression.rs`

- [ ] **Step 1: 添加范围计算函数**

在 `compression.rs` 中 `compress_tool_result` 之后追加：

```rust
// ---------------------------------------------------------------------------
// 压缩范围计算
// ---------------------------------------------------------------------------

/// 判断一条消息是否是纯粹的 tool_result 消息（不是用户主动输入）。
fn is_tool_result_message(msg: &Message) -> bool {
    msg.role == Role::User
        && msg.parts.iter().all(|p| matches!(p, Part::ToolResult { .. } | Part::ToolError { .. }))
}

/// 找到可以被压缩的消息范围。
///
/// 返回 `(start_idx, end_idx)` 包含性范围。
/// 保留最近 2 轮完整对话。
pub fn find_compression_range(messages: &[Message]) -> Option<(usize, usize)> {
    if messages.len() < 4 {
        return None;
    }

    let mut rounds_found = 0;
    let mut split_idx = messages.len();

    for (idx, msg) in messages.iter().enumerate().rev() {
        if msg.role == Role::User && !is_tool_result_message(msg) {
            rounds_found += 1;
            if rounds_found == 2 {
                split_idx = idx;
                break;
            }
        }
    }

    if rounds_found < 2 || split_idx == 0 {
        return None;
    }

    let safe_start = find_safe_split_point(messages, split_idx);

    if safe_start == 0 {
        return None;
    }

    Some((0, safe_start - 1))
}

/// 确保分割点不会切断 tool_use/tool_result 配对。
fn find_safe_split_point(messages: &[Message], mut split_idx: usize) -> usize {
    let tool_ids_in_range: HashSet<String> = messages[split_idx..]
        .iter()
        .filter_map(|msg| {
            msg.parts.iter().find_map(|part| match part {
                Part::ToolUse { id, .. } => Some(id.clone()),
                _ => None,
            })
        })
        .collect();

    let mut earliest_tool_result = split_idx;
    for (idx, msg) in messages[..split_idx].iter().enumerate().rev() {
        if let Some(tool_call_id) = msg.parts.iter().find_map(|p| match p {
            Part::ToolResult { tool_call_id, .. } | Part::ToolError { tool_call_id, .. } => {
                Some(tool_call_id.clone())
            }
            _ => None,
        }) {
            if tool_ids_in_range.contains(&tool_call_id) {
                earliest_tool_result = idx;
            }
        }
    }

    if earliest_tool_result < split_idx {
        for (idx, msg) in messages[..earliest_tool_result].iter().enumerate().rev() {
            if msg.parts.iter().any(|p| matches!(p, Part::ToolUse { .. })) {
                split_idx = idx;
                break;
            }
        }
    }

    split_idx
}
```

- [ ] **Step 2: 编译检查**

Run: `cargo check -p fi-code-core`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/agent/compression.rs
git commit -m "feat: add find_compression_range with tool pairing protection"
```

---

## Task 5: compression.rs — 增量压缩执行与 Subagent

**Files:**
- Modify: `crates/core/src/agent/compression.rs`

- [ ] **Step 1: 添加 subagent 和压缩执行函数**

在 `compression.rs` 末尾追加：

```rust
// ---------------------------------------------------------------------------
// 压缩 Subagent
// ---------------------------------------------------------------------------

const COMPRESSION_SYSTEM_PROMPT: &str = r#"你是一个对话摘要助手。你的任务是将一段对话历史压缩成简洁的摘要，供后续 AI 助手理解上下文。

摘要规则：
1. 保留所有关键决策、代码修改、文件路径、错误信息
2. 保留用户明确提出的需求和约束条件
3. 删除重复或冗余的中间推理步骤
4. 保留工具调用的关键结果（如 grep 找到了什么、bash 输出是什么）
5. 如果对话涉及多轮代码编辑，保留最终的代码状态描述
6. 摘要长度控制在 2000-4000 token 以内
7. 使用中文输出摘要（因为原始对话是中文）

输出格式：纯文本段落，不要加标题或标记。"#;

async fn run_compression_subagent(
    client: &dyn AIClient,
    messages_to_summarize: Vec<Message>,
) -> Result<String> {
    let mut subagent_messages = vec![Message::new(
        "compression".into(),
        Role::System,
        vec![Part::Text {
            text: COMPRESSION_SYSTEM_PROMPT.into(),
        }],
    )];
    subagent_messages.extend(messages_to_summarize);

    let mut runner = AgentRunner::new(client)
        .with_agent_type(AgentType::Build)
        .with_max_turns(1)
        .with_tool_filter(ToolFilter::deny_all());

    let result = runner.run(subagent_messages).await?;

    result
        .messages
        .last()
        .and_then(|msg| {
            msg.parts.iter().find_map(|p| match p {
                Part::Text { text } => Some(text.clone()),
                _ => None,
            })
        })
        .ok_or_else(|| anyhow::anyhow!("Compression subagent returned no text"))
}

// ---------------------------------------------------------------------------
// 增量压缩执行
// ---------------------------------------------------------------------------

/// 对会话历史执行增量压缩，返回新的 Summary 消息。
pub async fn compress_history(
    loop_state: &LoopState,
    client: &dyn AIClient,
) -> Result<Message> {
    let range = find_compression_range(&loop_state.messages)
        .ok_or_else(|| anyhow::anyhow!("No compressible range found"))?;

    let (start, end) = range;

    let mut to_compress = Vec::new();

    if let Some(ref summary) = loop_state.compression_summary {
        to_compress.push(summary.clone());
    }

    to_compress.extend(loop_state.messages[start..=end].iter().cloned());

    let summary_text = run_compression_subagent(client, to_compress).await?;

    let session_id = loop_state
        .messages
        .first()
        .map(|m| m.session_id.clone())
        .unwrap_or_default();

    Ok(Message::new(
        session_id,
        Role::User,
        vec![Part::Text { text: summary_text }],
    ))
}

/// 构建供 LLM 使用的消息视图。
pub fn build_llm_messages(loop_state: &LoopState) -> Vec<Message> {
    if let Some(ref summary) = loop_state.compression_summary {
        let mut result = Vec::new();
        result.push(summary.clone());

        if let Some((_, end)) = find_compression_range(&loop_state.messages) {
            result.extend(loop_state.messages[end + 1..].iter().cloned());
        }

        result
    } else {
        loop_state.messages.clone()
    }
}
```

- [ ] **Step 2: 编译检查**

Run: `cargo check -p fi-code-core`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/agent/compression.rs
git commit -m "feat: add compress_history and build_llm_messages with subagent"
```

---

## Task 6: 修改 LoopState 添加 compression_summary

**Files:**
- Modify: `crates/core/src/agent/agent.rs`

- [ ] **Step 1: 在 LoopState 中添加 compression_summary 字段**

```rust
pub struct LoopState {
    pub messages: Vec<Message>,
    pub turn_count: usize,
    pub transition_reason: Option<String>,
    pub token_usage: TokenUsage,
    /// 增量压缩摘要，仅在内存中存在，不持久化
    pub compression_summary: Option<Message>,
}

impl LoopState {
    pub fn new(messages: Vec<Message>) -> Self {
        Self {
            messages,
            turn_count: 1,
            transition_reason: None,
            token_usage: TokenUsage::default(),
            compression_summary: None,
        }
    }
}
```

- [ ] **Step 2: 编译检查并修复所有直接构造 LoopState 的地方**

Run: `cargo check -p fi-code-core`
Expected: 如果有直接构造 `LoopState { ... }` 的地方，会报错，需要添加 `compression_summary: None`

搜索直接构造：
```bash
grep -rn "LoopState {" crates/core/src/ --include="*.rs"
```

通常只有 `LoopState::new()` 被使用，不会有直接构造。如果发现有，修复。

- [ ] **Step 3: 重新编译**

Run: `cargo check -p fi-code-core`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/agent/agent.rs
git commit -m "feat: add compression_summary to LoopState"
```

---

## Task 7: execute_tool_calls 集成工具结果动态压缩

**Files:**
- Modify: `crates/core/src/tools/mod.rs`

- [ ] **Step 1: 修改 execute_tool_calls 函数签名和实现**

将 `execute_tool_calls` 函数签名从：
```rust
pub async fn execute_tool_calls(
    parts: &[Part],
    agent_type: fi_code_shared::dto::AgentType,
    on_tool_event: &mut Option<Box<dyn FnMut(crate::server::transport::sse::SseEvent) + Send>>,
) -> Vec<Part> {
```

改为：
```rust
pub async fn execute_tool_calls(
    parts: &[Part],
    agent_type: fi_code_shared::dto::AgentType,
    on_tool_event: &mut Option<Box<dyn FnMut(crate::server::transport::sse::SseEvent) + Send>>,
    is_aggressive: bool,
) -> Vec<Part> {
```

在构建 `Part::ToolResult` 的地方（line ~1133），将：
```rust
Part::ToolResult {
    tool_call_id: id,
    content,
    duration_ms: Some(duration_ms),
}
```

改为：
```rust
let compressed = crate::agent::compression::compress_tool_result(&content, is_aggressive);
Part::ToolResult {
    tool_call_id: id,
    content: compressed,
    duration_ms: Some(duration_ms),
}
```

- [ ] **Step 2: 编译检查**

Run: `cargo check -p fi-code-core`
Expected: 会有错误，因为调用方还没改参数

- [ ] **Step 3: 修改 run_one_turn 中调用 execute_tool_calls 的地方**

在 `crates/core/src/agent/agent.rs` 中，找到 `execute_tool_calls(` 调用，添加 `is_aggressive` 参数：

```rust
let is_aggressive = crate::agent::compression::should_compress(&state.messages);
let tool_results = execute_tool_calls(
    &turn.content_blocks,
    agent_type,
    on_tool_event,
    is_aggressive,
).await;
```

- [ ] **Step 4: 搜索所有其他 execute_tool_calls 调用并修复**

```bash
grep -rn "execute_tool_calls(" crates/ --include="*.rs"
```

如果其他文件也有调用，同样添加 `is_aggressive` 参数（通常只有 `agent.rs` 一处调用）。

- [ ] **Step 5: 重新编译**

Run: `cargo check -p fi-code-core`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/tools/mod.rs crates/core/src/agent/agent.rs
git commit -m "feat: integrate tool result dynamic compression in execute_tool_calls"
```

---

## Task 8: run_one_turn 集成上下文压缩检查

**Files:**
- Modify: `crates/core/src/agent/agent.rs`

- [ ] **Step 1: 在 run_one_turn 构建 prompt 之前插入压缩检查**

在 `crates/core/src/agent/agent.rs` 的 `run_one_turn` 函数中，找到 `let system_prompt = PromptBuilder::new()...` 行（约 line 337），在其之前插入：

```rust
    // === 上下文压缩检查 ===
    if crate::agent::compression::should_compress(&state.messages) {
        let needs_compress = state.compression_summary.is_none()
            || crate::agent::compression::find_compression_range(&state.messages).is_some();

        if needs_compress {
            log_info!(
                "[Compression] Triggered | messages={} | turn={}",
                state.messages.len(),
                state.turn_count
            );

            match crate::agent::compression::compress_history(state, client).await {
                Ok(summary) => {
                    state.compression_summary = Some(summary);
                    log_info!("[Compression] Completed successfully");
                }
                Err(e) => {
                    log_error!("[Compression] Failed: {}", e);
                    // 压缩失败不阻断主流程，继续用完整历史
                }
            }
        }
    }
```

- [ ] **Step 2: 修改 stream_message 调用，使用压缩视图**

找到 `client.stream_message(&system_prompt, &state.messages, &schema, ...)` 调用，改为：

```rust
let llm_messages = crate::agent::compression::build_llm_messages(state);

if let Err(e) = client
    .stream_message(&system_prompt, &llm_messages, &schema, &mut |chunk| {
        turn.process_chunk(chunk);
    })
    .await
```

以及 MCP 两步发现中的第二次调用：
```rust
if let Err(e) = client
    .stream_message(&system_prompt, &llm_messages, &schema, &mut |chunk| {
        turn.process_chunk(chunk);
    })
    .await
```

注意：MCP 两步发现中的调用也应该使用 `llm_messages` 而不是 `state.messages`。

- [ ] **Step 3: 编译检查**

Run: `cargo check -p fi-code-core`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/agent/agent.rs
git commit -m "feat: integrate context compression check in run_one_turn"
```

---

## Task 9: chat_api.rs 设置全局 Context Limit

**Files:**
- Modify: `crates/core/src/server/api/chat_api.rs`

- [ ] **Step 1: 在 run_agent_chat 中设置全局 context_limit**

在 `run_agent_chat` 函数中，获取 provider 和 config 后，设置全局 context limit：

```rust
    // 设置全局上下文限制（供压缩模块使用）
    if let Ok(provider) = state.provider.read() {
        if let Ok(config) = state.config.read() {
            let limit = provider.context_limit(&config);
            crate::agent::compression::set_context_limit(limit);
            log_info!("[Server] Context limit set to {}", limit);
        }
    }
```

这段代码放在 `set_task_provider` 之后、`// 获取或创建 LoopState` 之前。

- [ ] **Step 2: 编译检查**

Run: `cargo check -p fi-code-core`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/server/api/chat_api.rs
git commit -m "feat: set global context limit from provider config"
```

---

## Task 10: 编写单元测试

**Files:**
- Modify: `crates/core/src/agent/compression.rs`

- [ ] **Step 1: 在 compression.rs 末尾添加测试模块**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_tokens_ascii() {
        let text = "Hello world";
        let tokens = estimate_tokens(text);
        // 11 ASCII chars * 0.25 = 2.75 -> ceil = 3
        assert_eq!(tokens, 3);
    }

    #[test]
    fn test_estimate_tokens_mixed() {
        let text = "Hello 世界";
        let tokens = estimate_tokens(text);
        // 6 ASCII * 0.25 + 2 CJK * 0.67 = 1.5 + 1.34 = 2.84 -> ceil = 3
        assert_eq!(tokens, 3);
    }

    #[test]
    fn test_should_compress_below_threshold() {
        set_context_limit(100);
        let messages = vec![Message::new(
            "test".into(),
            Role::User,
            vec![Part::Text { text: "hi".into() }],
        )];
        assert!(!should_compress(&messages));
    }

    #[test]
    fn test_should_compress_above_threshold() {
        set_context_limit(100);
        // threshold = 85, need >= 85 estimated tokens
        // each char ~0.25 for ASCII, so ~340 chars = 85 tokens
        let long_text = "a".repeat(400);
        let messages = vec![Message::new(
            "test".into(),
            Role::User,
            vec![Part::Text { text: long_text }],
        )];
        assert!(should_compress(&messages));
    }

    #[test]
    fn test_compress_tool_result_short() {
        let content = "short";
        let result = compress_tool_result(content, false);
        assert_eq!(result, "short");
    }

    #[test]
    fn test_compress_tool_result_normal() {
        let content = "a".repeat(10_000);
        let result = compress_tool_result(&content, false);
        assert!(result.contains("truncated"));
        assert!(result.starts_with("a"));
    }

    #[test]
    fn test_compress_tool_result_aggressive() {
        let content = "b".repeat(5_000);
        let result = compress_tool_result(&content, true);
        assert!(result.contains("truncated"));
    }

    #[test]
    fn test_find_compression_range_insufficient_messages() {
        let messages = vec![
            Message::new("s".into(), Role::User, vec![Part::Text { text: "u1".into() }]),
            Message::new("s".into(), Role::Assistant, vec![Part::Text { text: "a1".into() }]),
        ];
        assert!(find_compression_range(&messages).is_none());
    }

    #[test]
    fn test_find_compression_range_basic() {
        let messages = vec![
            Message::new("s".into(), Role::User, vec![Part::Text { text: "u1".into() }]),
            Message::new("s".into(), Role::Assistant, vec![Part::Text { text: "a1".into() }]),
            Message::new("s".into(), Role::User, vec![Part::Text { text: "u2".into() }]),
            Message::new("s".into(), Role::Assistant, vec![Part::Text { text: "a2".into() }]),
            Message::new("s".into(), Role::User, vec![Part::Text { text: "u3".into() }]),
            Message::new("s".into(), Role::Assistant, vec![Part::Text { text: "a3".into() }]),
        ];
        let range = find_compression_range(&messages);
        assert!(range.is_some());
        let (start, end) = range.unwrap();
        assert_eq!(start, 0);
        // should compress u1/a1/u2/a2, keep u3/a3
        assert_eq!(end, 3);
    }

    #[test]
    fn test_find_safe_split_point_tool_pairing() {
        let messages = vec![
            Message::new("s".into(), Role::Assistant, vec![
                Part::ToolUse { id: "t1".into(), name: "bash".into(), arguments: serde_json::json!({}) },
            ]),
            Message::new("s".into(), Role::User, vec![
                Part::ToolResult { tool_call_id: "t1".into(), content: "result".into(), duration_ms: None },
            ]),
            Message::new("s".into(), Role::User, vec![Part::Text { text: "u2".into() }]),
            Message::new("s".into(), Role::Assistant, vec![Part::Text { text: "a2".into() }]),
        ];
        // split_idx = 2 (u2), tool_ids_in_range = empty
        // safe_start should be 2
        let safe = find_safe_split_point(&messages, 2);
        assert_eq!(safe, 2);
    }

    #[test]
    fn test_build_llm_messages_without_summary() {
        let state = LoopState::new(vec![
            Message::new("s".into(), Role::User, vec![Part::Text { text: "u1".into() }]),
        ]);
        let msgs = build_llm_messages(&state);
        assert_eq!(msgs.len(), 1);
    }

    #[test]
    fn test_build_llm_messages_with_summary() {
        let mut state = LoopState::new(vec![
            Message::new("s".into(), Role::User, vec![Part::Text { text: "u1".into() }]),
            Message::new("s".into(), Role::Assistant, vec![Part::Text { text: "a1".into() }]),
            Message::new("s".into(), Role::User, vec![Part::Text { text: "u2".into() }]),
            Message::new("s".into(), Role::Assistant, vec![Part::Text { text: "a2".into() }]),
            Message::new("s".into(), Role::User, vec![Part::Text { text: "u3".into() }]),
            Message::new("s".into(), Role::Assistant, vec![Part::Text { text: "a3".into() }]),
        ]);
        state.compression_summary = Some(Message::new(
            "s".into(),
            Role::User,
            vec![Part::Text { text: "summary".into() }],
        ));
        let msgs = build_llm_messages(&state);
        assert_eq!(msgs.len(), 3); // summary + u3 + a3
        assert!(matches!(msgs[0].parts[0], Part::Text { text: ref t if t == "summary" }));
    }
}
```

- [ ] **Step 2: 运行单元测试**

Run: `cargo test -p fi-code-core compression::tests`
Expected: All PASS

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/agent/compression.rs
git commit -m "test: add unit tests for context compression"
```

---

## Task 11: 运行全部测试并验证

- [ ] **Step 1: 运行全部测试**

Run: `cargo test`
Expected: 除预先存在的 `test_tool_call_web_fetch_success` 外全部通过

- [ ] **Step 2: 运行 Clippy**

Run: `cargo clippy -p fi-code-core -- -D warnings`
Expected: PASS（或只有预先存在的 warning）

- [ ] **Step 3: Commit**

```bash
git commit -m "feat: context compression complete - all tests pass"
```

---

## Spec Coverage Checklist

| Spec 要求 | 对应 Task |
|-----------|-----------|
| 工具结果超过阈值压缩（前1000+后2000） | Task 1 (constants), Task 3 (compress_tool_result), Task 7 (integration) |
| 每次 LLM 输入输出计算 token 数 | Task 3 (estimate_tokens, estimate_message_tokens) |
| 配置文件/预设模型有上下文大小就使用 | Task 2 (Provider::context_limit), Task 9 (set in chat_api) |
| 无配置默认 128K | Task 2 (DEFAULT_CONTEXT_LIMIT fallback) |
| usage >= 0.85 * limit 触发压缩 | Task 3 (should_compress), Task 8 (integration) |
| 系统提示词不压缩 | 天然成立（sys prompt 不在 messages 中，由 PromptBuilder 动态构建） |
| 区分压缩会话和实际会话 | Task 6 (LoopState.compression_summary), Task 8 (build_llm_messages) |
| 保存到持久化的是实际会话 | Task 6 (compression_summary 不持久化) |
| 启动 subagent 专门压缩 | Task 5 (compress_history + run_compression_subagent) |
| 从第一条压缩到最近4条 | Task 4 (find_compression_range) |
| 增量压缩 | Task 5 (compress_history 包含旧 summary) |
| ToolUse/ToolResult 配对保护 | Task 4 (find_safe_split_point) |

---

## Execution Options

**Plan complete.** Two execution options:

**1. Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

**Which approach?**
