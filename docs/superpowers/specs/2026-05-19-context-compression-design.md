# Context Compression Design

> 为 fi-code 的 Agent 对话系统引入上下文压缩机制，解决长会话场景下上下文窗口溢出的问题。

---

## 1. 背景与动机

当前 fi-code 的 Agent 对话系统存在以下限制：

1. **硬编码消息数限制**：`agent_loop` 中存在 `MAX_TURNS` 限制，且历史消息超过一定数量后直接丢弃，没有语义压缩
2. **工具结果无差别截断**：所有工具结果统一按 50,000 字符截断，不是基于上下文总量的智能压缩
3. **无 Token 级上下文管理**：无法感知当前上下文消耗了多少 token，无法 proactive 地触发压缩
4. **无模型上下文大小感知**：不同模型的上下文窗口差异巨大（4K ~ 200K），但系统统一对待

本设计引入一套完整的上下文压缩机制，包含：工具结果动态压缩、Token 估算与阈值检测、增量式历史摘要、Subagent 智能压缩。

---

## 2. 设计目标

1. **工具结果压缩**：单条超过阈值时进行头尾截断压缩，保留关键信息
2. **Token 估算**：当 LLM 不返回 usage 时，用字符级估算填补
3. **上下文大小感知**：从配置/预设模型读取 context limit，默认 128K
4. **智能触发**：当 estimated usage >= 85% context_limit 时触发压缩
5. **增量压缩**：每次压缩基于上一次的 summary + 增量消息，不重复压缩已压缩内容
6. **会话隔离**：压缩视图（给 LLM 的）与实际会话（内存+持久化）严格分离
7. **格式安全**：压缩时保证 tool_use/tool_result 配对完整性

---

## 3. 架构设计

### 3.1 核心模块

```
crates/core/src/agent/
├── compression.rs          # 新增：压缩核心逻辑
│   ├── token_estimation    # Token 估算函数
│   ├── threshold_check     # 阈值检测
│   ├── range_finder        # 压缩范围计算（含 tool 配对保护）
│   ├── incremental_compress # 增量压缩执行
│   └── subagent_runner     # 压缩 subagent 调用
├── agent.rs                # 修改：LoopState 扩展 + run_one_turn 集成
└── mod.rs                  # 修改：导出 compression 模块

crates/core/src/tools/
└── mod.rs                  # 修改：execute_tool_calls 中动态压缩 tool result
```

### 3.2 数据流

```
[User Input]
    |
    v
[run_one_turn]
    |
    +---> [should_compress?] ---> Yes ---> [compress_history]
    |                                         |
    |                                         v
    |                              [run_compression_subagent]
    |                                         |
    |                                         v
    |                              [更新 LoopState.compression_summary]
    |
    +---> [build_llm_messages]  # 使用压缩视图
    |           |
    |           v
    |    [summary, 最近2轮]
    |           |
    v           v
[stream_message] ---> LLM

[LoopState.messages] ---------------> [Session Persistence]
     (完整历史，永不压缩)                   (JSONL，实际会话)
```

---

## 4. 详细设计

### 4.1 Token 估算

```rust
/// 默认上下文限制：128K tokens
const DEFAULT_CONTEXT_LIMIT: u32 = 128_000;

/// 压缩触发阈值：85%
const COMPRESSION_THRESHOLD: f64 = 0.85;

/// 单字符 token 估算权重
const TOKEN_WEIGHT_ASCII: f64 = 0.25;      // ASCII ≈ 1/4 token
const TOKEN_WEIGHT_NON_ASCII: f64 = 0.67;  // CJK ≈ 2/3 token

/// 估算文本的 token 数
pub fn estimate_tokens(text: &str) -> u32 {
    text.chars()
        .map(|c| if c.is_ascii() { TOKEN_WEIGHT_ASCII } else { TOKEN_WEIGHT_NON_ASCII })
        .sum::<f64>()
        .ceil() as u32
}

/// 估算单条消息的 token 数
pub fn estimate_message_tokens(msg: &Message) -> u32 {
    msg.parts.iter().map(|part| match part {
        Part::Text { text } => estimate_tokens(text),
        Part::ToolResult { content, .. } => estimate_tokens(content),
        Part::ToolError { content, .. } => estimate_tokens(content),
        _ => 20,  // 其他 Part 类型固定开销
    }).sum()
}
```

### 4.2 上下文限制获取

```rust
/// 从 Provider 获取当前模型的上下文限制
pub fn get_context_limit(provider: &Provider) -> u32 {
    // 尝试从 Provider 当前模型的 ModelConfig.limit.context 读取
    // 若未配置则返回 DEFAULT_CONTEXT_LIMIT
}
```

### 4.3 阈值检测

```rust
pub fn should_compress(messages: &[Message], provider: &Provider) -> bool {
    let limit = get_context_limit(provider);
    let threshold = (limit as f64 * COMPRESSION_THRESHOLD) as u32;
    let estimated: u32 = messages.iter().map(estimate_message_tokens).sum();
    estimated >= threshold
}
```

### 4.4 工具结果动态压缩

```rust
const TOOL_RESULT_COMPRESS_THRESHOLD_NORMAL: usize = 8_000;
const TOOL_RESULT_COMPRESS_THRESHOLD_AGGRESSIVE: usize = 3_000;
const TOOL_RESULT_COMPRESS_HEAD: usize = 1_000;
const TOOL_RESULT_COMPRESS_TAIL: usize = 2_000;

pub fn compress_tool_result(content: &str, is_aggressive: bool) -> String {
    let threshold = if is_aggressive {
        TOOL_RESULT_COMPRESS_THRESHOLD_AGGRESSIVE
    } else {
        TOOL_RESULT_COMPRESS_THRESHOLD_NORMAL
    };
    
    if content.len() <= threshold {
        return content.to_string();
    }
    
    let head_end = content.char_indices()
        .nth(TOOL_RESULT_COMPRESS_HEAD)
        .map(|(i, _)| i)
        .unwrap_or(content.len());
    let tail_start = content.len().saturating_sub(TOOL_RESULT_COMPRESS_TAIL);
    
    format!(
        "{}\n\n... [{} chars truncated] ...\n\n{}",
        &content[..head_end],
        content.len() - head_end - (content.len() - tail_start),
        &content[tail_start..]
    )
}
```

**触发时机**：在 `execute_tool_calls` 中，构建 `Part::ToolResult` 之前调用。`is_aggressive` 由 `should_compress()` 决定。

**前后端分离**：SSE 发送给前端展示的 `display_content` 使用完整内容（或原有截断），只有发送给 LLM 的 `content` 应用此压缩。

### 4.5 LoopState 扩展

```rust
pub struct LoopState {
    pub messages: Vec<Message>,
    pub turn_count: usize,
    pub transition_reason: Option<String>,
    pub token_usage: TokenUsage,
    pub compression_summary: Option<Message>,  // 新增：增量摘要
}
```

- `messages`：完整实际会话，**永不压缩**，用于持久化
- `compression_summary`：增量压缩摘要，**仅在内存中**，不持久化

### 4.6 压缩范围计算

目标：保留 **最近2轮完整对话**，压缩之前的所有历史。

**重要**：`state.messages` 中**不包含系统提示词**。系统提示词由 `PromptBuilder` 在每轮 `run_one_turn` 中动态构建，通过 `stream_message` 的第一个参数传入。因此系统提示词天然不会被压缩，也无需在 `find_compression_range` 中特殊处理。

```rust
/// 找到可以被压缩的消息范围
/// 返回 (start_idx, end_idx) —— 包含性范围
fn find_compression_range(messages: &[Message]) -> Option<(usize, usize)> {
    if messages.len() < 4 {
        return None;  // 至少需要 2轮 = 4条（每轮 user + assistant）
    }
    
    // 从后往前找，定位"最近2轮对话"的边界
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
    
    // 确保 tool_use/tool_result 配对完整
    let safe_start = find_safe_split_point(messages, split_idx);
    
    if safe_start == 0 {
        return None;
    }
    
    Some((0, safe_start - 1))  // 从第一条消息 到 安全分割点之前
}
```

### 4.7 ToolUse/ToolResult 配对保护

```rust
/// 确保分割点不会切断 tool_use/tool_result 配对
fn find_safe_split_point(messages: &[Message], mut split_idx: usize) -> usize {
    // 收集保留范围内所有 tool_use 的 id
    let tool_ids_in_range: HashSet<String> = messages[split_idx..]
        .iter()
        .filter_map(|msg| {
            msg.parts.iter().find_map(|part| match part {
                Part::ToolUse { id, .. } => Some(id.clone()),
                _ => None,
            })
        })
        .collect();
    
    // 向前扫描，找到所有对应的 tool_result/tool_error
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
    
    // 如果 earliest_tool_result < split_idx，需要把对应的 tool_use 也包含进来
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

### 4.8 增量压缩执行

```rust
pub async fn compress_history(
    loop_state: &LoopState,
    client: &dyn AIClient,
) -> Result<Message> {
    let range = find_compression_range(&loop_state.messages)
        .ok_or_else(|| anyhow::anyhow!("No compressible range"))?;
    
    let (start, end) = range;
    
    // 构建待压缩的消息列表
    let mut to_compress = Vec::new();
    
    // 如果有旧 summary，先加入
    if let Some(ref summary) = loop_state.compression_summary {
        to_compress.push(summary.clone());
    }
    
    // 加入本次要压缩的消息范围
    to_compress.extend(loop_state.messages[start..=end].iter().cloned());
    
    // 运行压缩 subagent
    let summary_text = run_compression_subagent(client, to_compress).await?;
    
    Ok(Message::new(
        loop_state.messages[0].session_id.clone(),
        Role::User,
        vec![Part::Text { text: summary_text }],
    ))
}
```

### 4.9 构建 LLM 消息视图

```rust
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

### 4.10 压缩 Subagent

```rust
const COMPRESSION_SYSTEM_PROMPT: &str = r#"..."#;  // 见下文

async fn run_compression_subagent(
    client: &dyn AIClient,
    messages_to_summarize: Vec<Message>,
) -> Result<String> {
    let mut subagent_messages = vec![Message::new(
        "compression".into(),
        Role::System,
        vec![Part::Text { text: COMPRESSION_SYSTEM_PROMPT.into() }],
    )];
    subagent_messages.extend(messages_to_summarize);
    
    let mut runner = AgentRunner::new(client)
        .with_agent_type(AgentType::Build)
        .with_max_turns(1)
        .with_tool_filter(ToolFilter::deny_all());
    
    let result = runner.run(subagent_messages).await?;
    
    result.messages.last()
        .and_then(|msg| msg.parts.iter().find_map(|p| match p {
            Part::Text { text } => Some(text.clone()),
            _ => None,
        }))
        .ok_or_else(|| anyhow::anyhow!("Subagent returned no text"))
}
```

**Subagent 系统提示词**：

```
你是一个对话摘要助手。你的任务是将一段对话历史压缩成简洁的摘要，供后续 AI 助手理解上下文。

摘要规则：
1. 保留所有关键决策、代码修改、文件路径、错误信息
2. 保留用户明确提出的需求和约束条件
3. 删除重复或冗余的中间推理步骤
4. 保留工具调用的关键结果（如 grep 找到了什么、bash 输出是什么）
5. 如果对话涉及多轮代码编辑，保留最终的代码状态描述
6. 摘要长度控制在 2000-4000 token 以内
7. 使用中文输出摘要（因为原始对话是中文）

输出格式：纯文本段落，不要加标题或标记。
```

### 4.11 run_one_turn 集成点

在 `run_one_turn` 构建 prompt 之前插入压缩检查：

```rust
pub async fn run_one_turn<C: AIClient + ?Sized>(
    client: &C,
    state: &mut LoopState,
    agent_type: AgentType,
    // ...
) -> Result<bool> {
    // === 上下文压缩检查 ===
    if should_compress(&state.messages, &provider) {
        // 首次压缩，或上次压缩后又有新消息积累到阈值
        let needs_compress = state.compression_summary.is_none()
            || find_compression_range(&state.messages).is_some();
        
        if needs_compress {
            
            log_info!("[Compression] Triggered | messages={}", state.messages.len());
            
            match compress_history(state, client).await {
                Ok(summary) => {
                    state.compression_summary = Some(summary);
                    log_info!("[Compression] Completed");
                }
                Err(e) => {
                    log_error!("[Compression] Failed: {}", e);
                    // 失败不阻断主流程
                }
            }
        }
    }
    
    // 使用压缩视图构建 prompt
    let llm_messages = build_llm_messages(state);
    
    // ... 原有逻辑继续
}
```

---

## 5. Session 持久化行为

- `SessionManager::append_message()` 和 `save_session()` 只读写 `LoopState.messages`
- `compression_summary` **不持久化**，会话恢复后为空
- 这意味着：恢复会话后的第一轮，如果消息足够长，会重新触发压缩
- 这是可接受的，因为压缩摘要本身可以从完整历史重新生成

---

## 6. 错误处理

| 场景 | 处理策略 |
|------|----------|
| 压缩 subagent 失败 | 记录 error log，跳过压缩，继续用完整历史 |
| 无法找到可压缩范围 | 不压缩，等待更多消息 |
| tool_use/tool_result 配对无法保证 | 扩大保留范围，直到配对完整 |
| Token 估算偏差较大 | 可接受，因为阈值 85% 留有安全余量 |

---

## 7. 测试策略

### 7.1 单元测试（`agent/compression.rs`）

- `test_estimate_tokens_ascii`：纯 ASCII 文本估算
- `test_estimate_tokens_mixed`：中英文混合估算
- `test_should_compress_below_threshold`：低于阈值不触发
- `test_should_compress_above_threshold`：高于阈值触发
- `test_find_compression_range_basic`：基本范围计算
- `test_find_compression_range_with_tools`：带 tool 配对的范围计算
- `test_find_safe_split_point`：tool_use/tool_result 配对保护
- `test_build_llm_messages_with_summary`：压缩视图构建
- `test_build_llm_messages_without_summary`：无压缩时原样返回
- `test_compress_tool_result_normal`：正常模式压缩
- `test_compress_tool_result_aggressive`：激进模式压缩
- `test_compress_tool_result_short`：短内容不压缩

### 7.2 集成测试

- `test_compression_end_to_end`：完整对话流程中触发压缩
- `test_compression_preserves_session`：压缩后持久化文件仍为完整历史

---

## 8. 配置项

本设计**不引入新的用户配置项**，所有参数为代码常量：

| 常量 | 值 | 说明 |
|------|-----|------|
| `DEFAULT_CONTEXT_LIMIT` | 128_000 | 默认上下文限制 |
| `COMPRESSION_THRESHOLD` | 0.85 | 压缩触发阈值 |
| `TOOL_RESULT_COMPRESS_THRESHOLD_NORMAL` | 8_000 | 工具结果正常压缩阈值 |
| `TOOL_RESULT_COMPRESS_THRESHOLD_AGGRESSIVE` | 3_000 | 工具结果激进压缩阈值 |
| `TOOL_RESULT_COMPRESS_HEAD` | 1_000 | 工具结果保留头部字符数 |
| `TOOL_RESULT_COMPRESS_TAIL` | 2_000 | 工具结果保留尾部字符数 |

未来如需支持按模型/用户自定义，可将这些常量提升为 `Config` 中的可选字段。

---

## 9. 风险与缓解

| 风险 | 缓解措施 |
|------|----------|
| 压缩 subagent 调用增加延迟 | 仅在 >= 85% 阈值时触发，非每轮；subagent 单轮执行，max_turns=1 |
| 压缩摘要质量不佳导致上下文丢失 | 系统提示词明确要求保留关键决策、文件路径、错误信息 |
| Token 估算偏差导致过早/过晚触发 | 85% 阈值留有 15% 安全余量；估算偏向保守（中文按 2/3 token） |
| 增量压缩累积失真 | 每次从完整历史重新计算范围，summary 内容本身会包含旧 summary，减少累积误差 |
| 恢复会话后重新压缩 | 可接受，因为完整历史在持久化文件中，摘要可重新生成 |

---

## 10. 相关文档

- `docs/superpowers/specs/2026-05-18-agent-system-design.md` — Agent 循环设计
- `docs/superpowers/specs/2026-05-18-slash-submenu-design.md` — 相关配置系统
