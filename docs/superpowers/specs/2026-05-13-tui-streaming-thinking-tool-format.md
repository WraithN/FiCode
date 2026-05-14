# TUI 流式展示 Thinking + ToolUse/ToolResult 格式化设计

## 背景

当前 TUI 聊天区存在两个体验问题：

1. **Thinking/Reasoning 过程不可见**：kimi-k2.5 等模型在生成回复前会输出思考过程（`reasoning_content`），但 `openapi_client.rs` 的 SSE 解析器完全丢弃了这部分内容。用户在 LLM "思考"期间看不到任何输出，产生"卡住/无响应"的感觉。

2. **ToolUse/ToolResult 展示为原始 JSON**：工具调用卡片直接显示 `{"path":"...","content":"..."}`，工具结果也展示原始字符串/JSON，可读性差。

## 目标

1. **Thinking 过程实时流式展示**：将 LLM 的 `reasoning_content` 增量实时推送到 TUI，以灰色折叠卡片形式展示。
2. **ToolUse 增量实时更新**：在 LLM 生成 `tool_calls` 参数的过程中，实时更新工具调用卡片（用户能看到参数逐渐被填充）。
3. **ToolUse/ToolResult 格式化渲染**：根据工具类型和参数，生成人类可读的摘要，不再展示原始 JSON。

## 设计方案（方案 B）

### 1. SSE 解析层：处理 reasoning_content + 实时 flush tool_calls

**文件：** `crates/core/src/provider/client/openapi_client.rs`

#### 1.1 处理 `delta.reasoning_content`

在 `handle_openai_delta` 中，除了处理 `delta.content`，新增处理 `delta.reasoning_content`：

```rust
if let Some(thinking) = delta.get("reasoning_content").and_then(|v| v.as_str()) {
    if !thinking.is_empty() {
        on_chunk(Chunk {
            content: ChunkContent::Think(thinking.to_string()),
        });
    }
}
```

> 注意：`reasoning_content` 和 `content` 可能在同一个 delta 中同时出现（thinking 结束后立即开始生成正文），也可能交替出现。

#### 1.2 实时 flush tool_calls 增量

当前 `tool_calls` delta 累积在 `index_to_tool: HashMap<usize, (Option<String>, Option<String>, String)>` 中，只在 `finish_reason = "tool_calls"` 时 flush。

**改为**：每次收到 `tool_calls` delta 并更新 `index_to_tool` 后，立即将当前累积状态转换为 `ChunkContent::ToolUse` 回传：

```rust
for tool in tools {
    update_openai_tool_call_delta(tool, index_to_tool);
    // 新增：实时 flush 当前累积状态
    if let Some((id, name, args)) = index_to_tool.get(&index) {
        if id.is_some() && name.is_some() {
            on_chunk(Chunk {
                content: ChunkContent::ToolUse(Part::ToolUse {
                    id: id.clone().unwrap_or_default(),
                    name: name.clone().unwrap_or_default(),
                    arguments: args.clone(),
                }),
            });
        }
    }
}
```

> `arguments` 可能是**不完整的 JSON**（例如 `{"path"`）。接收方（TUI）需要能处理部分 JSON。

### 2. API 层：将 Think + ToolUse 增量转发到 SSE

**文件：** `crates/core/src/server/api/chat_api.rs`

在 `agent_loop` 的 `on_chunk` 回调中，`ChunkContent::Think` 和 `ChunkContent::ToolUse` 都需要通过 SSE 发送给客户端：

```rust
// 已有逻辑中补充 Think 和 ToolUse 的处理
match chunk.content {
    ChunkContent::Text(text) => { /* 现有逻辑 */ }
    ChunkContent::Think(thinking) => {
        let _ = sse_sender.try_send(SseEvent::Part {
            part: Part::Reasoning { thinking, signature: None },
        });
    }
    ChunkContent::ToolUse(part) => {
        let _ = sse_sender.try_send(SseEvent::Part { part });
    }
    // ...
}
```

### 3. TUI 层：处理增量式 ToolUse 更新

**文件：** `crates/tui/src/components/chat.rs`

当前 `handle_sse_event` 对 `SseEvent::Part { Part::ToolUse }` 的处理是直接 `push(part.clone())`。这会导致同一个工具调用被重复 push 多次。

**改为**：按 `id` 匹配，更新已有的 `ToolUse` Part，而不是重复 push：

```rust
SseEvent::Part { part } => {
    match part {
        Part::ToolUse { id, .. } => {
            // 查找同 id 的 ToolUse 并更新，不存在则 push
            if let Some(existing) = last_turn.parts.iter_mut().find_map(|p| {
                if let Part::ToolUse { id: existing_id, .. } = p {
                    if existing_id == id { Some(p) } else { None }
                } else { None }
            }) {
                *existing = part.clone();
            } else {
                last_turn.parts.push(part.clone());
            }
        }
        Part::ToolResult { .. } => {
            last_turn.parts.push(part.clone());
        }
        // ...
    }
}
```

> `ToolResult` 仍是一次性 push（工具执行完成后才收到）。

### 4. 渲染层：格式化 ToolUse / ToolResult

#### 4.1 ToolCallRenderer：人类可读摘要

**文件：** `crates/tui/src/components/part_renderer/tool_call.rs`

根据 `name` 解析 `arguments` JSON，生成摘要：

| 工具名 | 展示格式 |
|--------|----------|
| `write` / `edit` | `📝 写入文件: {path}` |
| `read` / `read_file` | `📖 读取文件: {path}` |
| `bash` | `⚡ 执行命令: {command}` |
| `grep` | `🔍 搜索: {pattern} in {path}` |
| `web_fetch` | `🌐 抓取网页: {url}` |
| `git_status` | `📋 Git 状态` |
| `git_diff` | `📋 Git Diff` |
| `git_add` | `➕ Git Add: {path}` |
| `git_commit` | `💾 Git Commit: {message}` |
| 其他 | `🔧 {name}: {arguments 摘要}` |

参数中的 `content` 字段（通常很长）不直接展示，而是显示 `[{len} bytes]`。

#### 4.2 ToolResultRenderer：格式化结果

**文件：** `crates/tui/src/components/part_renderer/tool_result.rs`

- 如果 `content` 是有效的 JSON，尝试提取关键字段（如 `path`、`success`、`output`、`error`）进行格式化展示
- 如果 `content` 是命令输出（多行文本），直接展示（保持现有行为）
- 添加颜色区分：成功结果用绿色边框，错误结果用红色边框

### 5. ThinkingRenderer 展示优化

**文件：** `crates/tui/src/components/part_renderer/thinking.rs`

- 已有 `ThinkingRenderer`，展示带边框的灰色 "▼ Thinking" 卡片
- 当 thinking 过程结束时（收到第一个非空 `content` delta 时），空的 `Reasoning` Part 会被 `handle_sse_event` 移除
- 无需额外改动，现有机制已支持

## 边界情况

1. **部分 JSON 的 ToolUse**：实时 flush 出来的 `arguments` 可能是不完整 JSON。`ToolCallRenderer` 在解析失败时回退到展示原始字符串（截断到 100 字符）。
2. **reasoning_content 和 content 交替出现**：`handle_openai_delta` 需要支持同一个 delta 中同时包含两者。
3. **ToolUse 更新频率**：每次 tool_calls delta 都 flush 可能导致 SSE 事件过于频繁（但通常 tool_calls 只有 1-3 个 chunk，频率可控）。
4. **空 thinking**：某些模型不返回 `reasoning_content`，`handle_openai_delta` 中需要跳过空字符串。

## 测试策略

- 单元测试：`ToolCallRenderer` 的摘要生成逻辑（不同工具名/参数的格式化）
- BDD 测试：补充场景验证 thinking 卡片和格式化工具卡片的出现
- 手动测试：使用 kimi-k2.5 验证 thinking 过程是否实时展示

## 不涉及的改动

- 不改 SSE 协议结构（仍使用 `SseEvent::Part`）
- 不改 `Component` trait
- 不改动 Anthropic 客户端（当前需求只针对 OpenAI 兼容接口的 `reasoning_content`）
