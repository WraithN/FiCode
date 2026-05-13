# MessageV2 Part 架构设计文档

> 日期：2026-05-12
> 状态：已批准
> 目标：将 fi-code 的会话消息架构从扁平 SSE 事件升级为统一的 Part 列表模型，实现前后端语义对齐，并支撑 WaveMarker、Usage 统计、ShellOutput 等新组件。

---

## 1. 设计背景

当前架构中，后端 `Message` 已采用 `Vec<Part>` 结构（`Text`/`Image`/`ToolUse`/`ToolResult`/`Reasoning`），但 SSE 传输层仍使用扁平事件（`SseEvent::Message`/`ToolUse`/`ToolResult`/`Usage` 等独立变体）。这导致：

- 前后端消息模型不一致（后端是 Part 列表，TUI 是 Card 列表）
- 新增 Part 类型需要同时修改 SSE 枚举和 TUI 渲染逻辑，扩展成本高
- `ToolResult { is_error: bool }` 将成功/失败混在同一变体中，语义不清晰
- 缺乏系统级元数据 Part（如 WaveMarker）和统计 Part（如 Usage）

本设计将统一前后端消息模型，所有非流式内容通过 `SseEvent::Part` 传输，TUI 侧建立 `Part → Renderer` 映射。

---

## 2. Part 枚举扩展

### 2.1 定义（`crates/core/src/session/message.rs`）

```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Part {
    /// 纯文本内容（主回复文本）
    Text { text: String },

    /// 图片内容
    Image { source: ImageSource },

    /// 工具调用请求（由 Assistant 发起）
    ToolUse {
        id: String,
        name: String,
        arguments: serde_json::Value,
    },

    /// 工具执行成功结果
    ToolResult {
        tool_call_id: String,
        content: String,
    },

    /// 工具执行失败（独立错误类型，替代 ToolResult { is_error: true }）
    ToolError {
        tool_call_id: String,
        content: String,
        error_message: String,
    },

    /// 推理/思考过程（对应设计文档中的 Thinking）
    Reasoning {
        thinking: String,
        signature: Option<String>,
    },

    /// WaveMarker：Agent 迭代轮次的系统元数据标记
    WaveMarker {
        step: u32,
        total: Option<u32>,
        git_snapshot: Option<String>,
        timestamp: u64,
        delta_tokens: TokenUsage,
    },

    /// Usage：单条消息的 Token/LAT/费用统计
    Usage {
        input_tokens: u32,
        output_tokens: u32,
        latency_ms: u32,
        cost: Option<f64>,
    },
}

/// Token 用量结构（复用现有定义）
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
}
```

### 2.2 变更说明

| 变更项 | 说明 |
|--------|------|
| 删除 `ToolResult.is_error` | 成功结果走 `ToolResult`，失败走 `ToolError`，语义分离 |
| 新增 `ToolError` | 包含 `tool_call_id`、`content`（原始错误输出）、`error_message`（结构化错误信息） |
| 新增 `WaveMarker` | 系统元数据，不由 LLM 输出，由 Agent Runtime 在每次迭代开始时插入 |
| 新增 `Usage` | 消息级统计，位于 AI 消息气泡底部 |
| 保留 `Image` | TUI 侧先支持显示路径/URL，后续可扩展图片预览 |

---

## 3. SSE 传输协议

### 3.1 `SseEvent` 重构（`crates/core/src/server/transport/sse.rs`）

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SseEvent {
    /// 流式文本（逐 token 追加，保留轻量传输）
    #[serde(rename = "message")]
    Message { content: String },

    /// 统一 Part 传输（所有非流式内容）
    #[serde(rename = "part")]
    Part { part: Part },

    /// 任务进度（非消息内容类，保留独立变体）
    #[serde(rename = "task_progress")]
    TaskProgress {
        plan_id: String,
        tasks: Vec<TaskProgressItem>,
    },

    /// 错误通知
    #[serde(rename = "error")]
    Error { message: String },

    /// 对话结束
    #[serde(rename = "done")]
    Done { session_id: String },
}
```

### 3.2 删除的 SSE 变体

以下变体被删除，功能由 `SseEvent::Part` 替代：

- `SseEvent::ToolUse` → `SseEvent::Part { part: Part::ToolUse { ... } }`
- `SseEvent::ToolResult` → `SseEvent::Part { part: Part::ToolResult { ... } }`
- `SseEvent::Usage` → `SseEvent::Part { part: Part::Usage { ... } }`
- `SseEvent::MessageDetails` → 不再批量发送详情，改为实时 Part 流

### 3.3 传输规则

| 内容类型 | SSE 变体 | 说明 |
|----------|----------|------|
| LLM 流式文本 | `Message { content }` | 逐 token 追加，最轻量 |
| ToolUse | `Part { part: ToolUse }` | 完整 Part 包装 |
| ToolResult | `Part { part: ToolResult }` | 完整 Part 包装 |
| ToolError | `Part { part: ToolError }` | 完整 Part 包装 |
| Reasoning | `Part { part: Reasoning }` | 完整 Part 包装 |
| WaveMarker | `Part { part: WaveMarker }` | 系统插入，完整 Part 包装 |
| Usage | `Part { part: Usage }` | 消息结束时发送 |

### 3.4 单条 AI 消息的 SSE 事件序列

```
Part { WaveMarker { step: 1, total: None, ... } }     ← 系统插入
Message { content: "Hel" }
Message { content: "lo" }
Message { content: "，我需要先" }
...
Part { Reasoning { thinking: "分析项目结构..." } }      ← LLM 输出
Part { ToolUse { id: "tu1", name: "read_file", ... } }  ← LLM 输出
Part { ToolResult { tool_call_id: "tu1", content: "..." } }  ← 系统执行后
Message { content: "根据代码分析" }
...
Part { Usage { input_tokens: 5400, output_tokens: 2100, latency_ms: 2400, cost: 0.008 } }
```

---

## 4. TUI 渲染架构

### 4.1 核心决策：Part → Renderer 映射

TUI 侧废弃独立的 `CardKind` 分类，建立统一的 `PartRenderer` trait：

```rust
/// Part 渲染器 trait
pub trait PartRenderer {
    /// 计算该 Part 在给定宽度下的渲染高度（行数）
    fn height(&self, part: &Part, width: u16) -> u16;

    /// 在指定区域渲染该 Part
    fn draw(&self, frame: &mut Frame, area: Rect, part: &Part, theme: &Theme);

    /// 处理交互事件（可选），返回是否消耗了事件
    fn handle_event(&mut self, part: &mut Part, event: &Event) -> bool {
        false
    }
}
```

### 4.2 渲染器注册表

```rust
pub struct PartRendererRegistry {
    renderers: HashMap<&'static str, Box<dyn PartRenderer>>,
}

impl PartRendererRegistry {
    pub fn new() -> Self {
        let mut registry = Self { renderers: HashMap::new() };
        registry.register("text", Box::new(TextRenderer));
        registry.register("reasoning", Box::new(ThinkingRenderer));
        registry.register("tool_use", Box::new(ToolCallRenderer));
        registry.register("tool_result", Box::new(ToolResultRenderer));
        registry.register("tool_error", Box::new(ToolErrorRenderer));
        registry.register("wave_marker", Box::new(WaveMarkerRenderer));
        registry.register("usage", Box::new(UsageRenderer));
        registry.register("image", Box::new(ImageRenderer));
        registry
    }
}
```

### 4.3 渲染映射表

| Part 变体 | 渲染器 | 渲染形式 | 默认状态 |
|-----------|--------|---------|----------|
| `WaveMarker` | `WaveMarkerRenderer` | 消息气泡顶部的单行步骤标记 | 展开 |
| `Reasoning` | `ThinkingRenderer` | 可折叠的思考卡片 | 折叠 |
| `ToolUse` | `ToolCallRenderer` | 根据 `name` 分发子渲染器 | 展开（仅标题） |
| `ToolResult` | `ToolResultRenderer` | 根据对应 ToolUse 的 `name` 分发 | 折叠 |
| `ToolError` | `ToolErrorRenderer` | 红色错误卡片 | 展开 |
| `Text` | `TextRenderer` | 主回复文本（Markdown 简单渲染） | 展开 |
| `Usage` | `UsageRenderer` | 消息气泡底部右对齐统计行 | 展开 |
| `Image` | `ImageRenderer` | 显示图片路径/URL/尺寸 | 展开 |

### 4.4 ToolUse/ToolResult 分发逻辑

`ToolCallRenderer` 根据 `ToolUse.name` 选择具体的子渲染器：

| Tool Name | 渲染形式 |
|-----------|---------|
| `read_file`, `view`, `cat` | `FilePreviewRenderer` |
| `write_file`, `edit_file`, `apply_diff`, `patch` | `DiffRenderer` |
| `shell`, `execute`, `run` | `ShellOutputRenderer` |
| `create_todo`, `update_todo` | `TaskListRenderer` |
| 其他 | `GenericToolRenderer`（显示 JSON 参数/结果） |

---

## 5. WaveMarker 详细设计

### 5.1 职责

WaveMarker 是 Agent Runtime 在每次 LLM 调用前插入的系统元数据，承担三项功能：

1. **阶段标记**：告知用户 AI 正在进行第几轮思考-行动循环
2. **Git 快照**：每步开始时执行 `git write-tree`，生成树对象哈希，作为回滚锚点
3. **成本归集**：记录该步骤的 Token 增量与 LAT 累计

### 5.2 触发时机

- 用户发送消息后，LLM 首次响应前 → Step 1
- 工具执行完毕，LLM 再次生成前 → Step N+1
- LLM 直接返回 Text（无工具调用）→ 最后一轮，事后回填 total

### 5.3 TUI 渲染格式

```
Step 3/5 [ab12cd3] ΔTOK:⬆️1.2k⬇️0.8k · LAT:1.1s
```

- `Step 3/5`：绿色加粗，`total` 为 `None` 时显示 `Step 3/?`
- `[ab12cd3]`：Git 快照哈希，绿色，可交互
- `ΔTOK:⬆️1.2k⬇️0.8k`：步骤 Token 增量，暗灰
- `LAT:1.1s`：步骤耗时，暗灰

### 5.4 交互

- `g`（在 WaveMarker 行聚焦时）：跳转到该快照的只读浏览模式
- `r`：从该 Wave 回滚并重试（`git checkout` + 重发后续请求）

### 5.5 后端实现

```rust
// agent_loop 伪代码
async fn agent_loop(...) {
    let mut step = 1;
    loop {
        // 1. 记录快照和 Token 基数
        let snapshot = git_write_tree().await.ok();
        let token_baseline = session.token_usage.clone();

        // 2. 创建新 AI Message，插入 WaveMarker
        let mut msg = Message::new(session_id, Role::Assistant, vec![]);
        msg.parts.push(Part::WaveMarker {
            step,
            total: None,
            git_snapshot: snapshot,
            timestamp: current_timestamp_ms(),
            delta_tokens: TokenUsage::default(),
        });

        // 3. 调用 LLM，流式追加 Part
        let stream = client.stream_message(...).await?;
        while let Some(chunk) = stream.next().await {
            // 将 chunk 转换为 Part 并追加
            process_chunk(chunk, &mut msg.parts)?;
            // 发送 SSE
            sse_sender.send(SseEvent::Part { part: ... }).await?;
        }

        // 4. 回填 WaveMarker 的 delta_tokens
        if let Some(Part::WaveMarker { delta_tokens, .. }) = msg.parts.first_mut() {
            *delta_tokens = TokenUsage {
                prompt_tokens: session.token_usage.prompt_tokens - token_baseline.prompt_tokens,
                completion_tokens: session.token_usage.completion_tokens - token_baseline.completion_tokens,
            };
        }

        // 5. 执行工具（如果有 ToolUse）
        if has_tool_calls(&msg.parts) {
            execute_tools(&msg.parts, &sse_sender).await?;
            step += 1;
            continue;
        } else {
            // 回填 total
            if let Some(Part::WaveMarker { total, .. }) = msg.parts.first_mut() {
                *total = Some(step);
            }
            break;
        }
    }
}
```

---

## 6. 中间信息流组件规范

### 6.1 FilePreview（读文件）

**触发：** `read_file`, `view`, `cat`

**结构：**
- 标题：`File ── {path} ── [{total_lines} lines]`
- 行号：右对齐，宽度按文件总行数动态计算
- 内容：等宽字体，简单语法高亮（关键字/字符串/注释分色）
- 截断：固定展示前 10 行，第 10 行后显示 `... 共 N 行，此处省略 M 行 ...`
- 二进制文件：`[Binary file] · 4.2KB`

**折叠策略：** 默认展开（用户需要立刻看到文件内容），Enter 折叠为单行摘要。

---

### 6.2 Diff（代码变更）

**触发：** `write_file`, `edit_file`, `apply_diff`, `patch`

**ToolCall 卡片：**
```
┌─ edit_file ───────────────────────────┐
│ src/memory.rs · +12 -3 · 3 chunks     │
│ [████████████████████] 100% 89ms      │
└───────────────────────────────────────┘
```

**Diff 卡片：**
- 标题：`Diff ── +{added} -{removed} ── {path}`
- 格式：统一 diff（Unified），TUI 宽度有限不做 side-by-side
- 行首标记与颜色：
  - `-` 删除行：红色文字（红绿色盲用户可通过 `-` 符号识别）
  - `+` 新增行：绿色文字
  - ` ` 上下文行：暗灰色文字
- 截断：默认展示前 20 行变更，大段无变更区域用 `...` 折叠

**交互：** Enter 展开/折叠；Pending 状态下 `y` 批准，`n` 拒绝。

---

### 6.3 ShellOutput（命令执行）

**触发：** `shell`, `execute`, `run`, `cargo test`, `npm install`

**ToolCall 卡片：**
- 必须展示完整命令原文，不截断、不省略参数
- 命令名高亮，参数等宽显示
- 超长命令允许卡片内水平滚动

**Output 卡片：**
- 标题：`Output ── [exit:{code}] ── {lines} lines · {duration}s`
- 退出码语义：
  - `[exit:0]`：绿色
  - `[exit:1-255]`：红色
  - `[timeout]`：黄色
  - `[killed]`：红色
- 内容显示最后 15 行（尾部通常包含结果摘要）
- 被截断时顶部显示 `... 共 N 行，此处显示最后 15 行 ...`
- `stderr` 行前缀 `E│`，红色文字
- 空输出：`[No output]`

**交互：** Enter 展开/折叠；`o` 打开完整输出日志。

---

### 6.4 TaskList（任务列表）

**触发：** `create_todo`, `update_todo`

**格式：**
- 标题：`Tasks ── {total} items`
- 状态符号：
  - `☑` 已完成（绿色）
  - `☐` 待办（暗灰）
  - `▣` 进行中（黄色）
  - `!` 高优先级（红色前缀）
- 底部统计：`{completed}/{total} completed · {pending} pending`

**交互：** Enter 展开/折叠；Space 手动勾选/取消。

---

### 6.5 ToolError（错误卡片）

**结构：**
- 标题：`❌ {tool_name} failed`
- 内容：`error_message`（结构化错误）
- 详情：折叠后显示 `content`（原始错误输出）
- 边框/文字：红色系

---

### 6.6 Usage（统计行）

**位置：** 每条 AI 消息气泡最底部，右对齐，暗灰色。

**格式：**
```
⬆️5.4k ⬇️2.1k · LAT:2.4s · $0.008
```

- `⬆️`：Input Tokens（含上下文）
- `⬇️`：Output Tokens
- `LAT`：该消息从请求发出到流式输出结束的总耗时
- `$0.008`：估算费用（后端返回则显示，否则隐藏）

---

## 7. 状态栏改造

### 7.1 标准版格式（宽度 ≥ 100）

```
FiCode │ CTX:[████████░░] │ TOK:⬆️24k⬇️18k │ LAT:2.4s │ MDL:kimi-k2.5 │ 10:06
```

### 7.2 紧凑版格式（宽度 < 100）

```
FiCode [████████░░] │ TOK:18k │ LAT:2.4s │ k2.5 │ 10:06
```

### 7.3 字段语义与颜色

| 字段 | 全称 | 数据内容 | 颜色语义 |
|------|------|----------|----------|
| FiCode | 应用标识 | 固定字符串 | 主文字色，加粗 |
| CTX | Context Window Usage | `[████████░░]` 进度条 + 可选 `24k/128k` | 绿（<50%）→ 黄（50-80%）→ 红（>80%） |
| TOK | Tokens Consumed | `⬆️{input} ⬇️{output}` | 正常暗灰；满载时变黄/红 |
| LAT | Latency | 最近一次请求首 Token 返回耗时，如 `2.4s` | 正常暗灰；>10s 变黄；>30s 变红 |
| MDL | Model | `kimi-k2.5` / `claude-3.7` | AI 强调色（绿） |
| 10:06 | Clock | 本地时间 HH:MM | 暗灰 |

### 7.4 进度条语义

- 归属：`CTX` 字段，表示上下文窗口占用率
- 计算：`已填充格数 = ceil(已用 Token / 上限 Token × 10)`
- 颜色：进度条本身随占用率变色
- 空间充裕时：进度条右侧追加数字 `24k/128k`
- 空间紧张时：仅保留进度条

### 7.5 状态语义化（边框颜色）

- 边框绿色：一切正常
- 边框黄色脉冲：Agent 忙碌中
- 边框红色：网络断开或执行错误
- 网络异常时：插入 `[RECONNECTING]` 或 `[OFFLINE]` 标签

### 7.6 空间不足降级策略（宽度 < 80）

1. 保留：`FiCode`、`[████████░░]`、`LAT`、`MDL`
2. 折叠：`TOK` 只显示输出方向 `TOK:⬇️18k`
3. 隐藏：时间
4. 极限：`FiCode [████░░░░░░] │ LAT:2.4s │ k2.5`

---

## 8. 交互规范

| 按键 | 作用域 | 功能 |
|------|--------|------|
| `j` / `k` | 全局 | 上下滚动消息列表 |
| `Enter` | Thinking / ToolResult / Diff / ShellOutput / TaskList | 展开/折叠卡片 |
| `Space` | TaskList | 勾选/取消任务 |
| `y` | ToolCall Pending（Diff / Shell） | 批准执行 |
| `n` | ToolCall Pending | 拒绝执行 |
| `g` | WaveMarker | 跳转到该步骤 Git 快照（只读浏览） |
| `r` | WaveMarker | 从该步骤回滚重试 |
| `o` | ShellOutput | 打开完整输出日志 |
| `/` 或 `:` | 全局 | 打开命令面板（切换主题等） |
| `?` | 全局 | 显示帮助浮层 |
| `Ctrl+C` | 全局 | 复制当前消息或选中区域 |

---

## 9. 实施计划

### Phase 1：骨架改造（必须先完成）

1. **扩展 `Part` 枚举**
   - 新增 `ToolError`、`WaveMarker`、`Usage`
   - 删除 `ToolResult.is_error`
   - 更新所有使用 `ToolResult.is_error` 的代码

2. **重构 `SseEvent`**
   - 新增 `SseEvent::Part { part: Part }`
   - 删除 `ToolUse`、`ToolResult`、`Usage`、`MessageDetails` 变体
   - 保留 `Message`、`TaskProgress`、`Error`、`Done`

3. **后端 SSE 发送改造**
   - `agent_loop` 和 `runner` 中将 `ToolUse`/`ToolResult`/`Usage` 改为 `SseEvent::Part`
   - `send_last_assistant_details` 改为实时 Part 流

4. **TUI SSE 接收改造**
   - `client.rs` 解析 `SseEvent::Part`
   - `app.rs` 将 `SseEvent::Part` 路由到 `Chat::handle_part_event`

5. **状态栏改造**
   - 字段重命名（耗时 → LAT、IN/OUT → TOK）
   - 添加 CTX 进度条语义
   - 紧凑版/极限版降级策略

### Phase 2：TUI 渲染器实现（可并行开发）

1. **`PartRenderer` trait 和注册表**
2. **`WaveMarkerRenderer`**（步骤标记渲染 + 交互）
3. **`UsageRenderer`**（消息级统计行）
4. **`ThinkingRenderer`**（迁移现有 Thinking 卡片）
5. **`ToolCallRenderer` + 分发逻辑**
   - `FilePreviewRenderer`
   - `DiffRenderer`
   - `ShellOutputRenderer`
   - `TaskListRenderer`
6. **`ToolErrorRenderer`**
7. **`ImageRenderer`**
8. **消息高度动态计算**（基于 Part 列表的逐条高度累加）

### Phase 3：WaveMarker 后端集成

1. **`agent_loop` 插入 WaveMarker**
   - 每次 LLM 调用前插入 `Part::WaveMarker`
   - 集成 `git write-tree` 快照
   - Token 基数记录与 delta 回填

2. **WaveMarker 交互**
   - `g`：跳转到快照（只读浏览）
   - `r`：回滚并重试

3. **会话级 Token/LAT 累计器**
   - `SessionState` 维护累计值
   - 驱动状态栏实时刷新

---

## 10. 关键禁忌

- 默认主题不使用叙事化词汇（保持 Step/Tool Call/Token/Files）
- Diff 展示不可仅依赖红绿色彩，必须通过 `+` / `-` 符号保证色盲可识别
- 命令执行必须展示完整原文，禁止参数截断
- WaveMarker 只在 Agent 模式插入，流式闲聊模式不插入
- 状态栏禁止裸数字，每个数字前必须有字段标签
