# TUI 信息流卡片化设计文档

> 设计日期：2026-05-11  
> 设计目标：将 TUI 会话主界面从纯文本流升级为卡片化信息流，实现即时反馈、工具调用可视化、文件变更对比和错误重试。

---

## 1. 设计概述

### 1.1 背景

当前 TUI 的会话界面以纯文本段落（`Paragraph`）渲染对话历史。虽然文本已支持 SSE 流式增量显示，但工具调用、思考过程、任务计划等结构化信息被折叠在可展开的详情块中，用户无法直观看到 AI 的执行流程。

### 1.2 目标

1. **即时反馈**：用户输入后立即显示 AI 会话占位卡片，展示动态 "Thinking..." 动画
2. **卡片化信息流**：每一轮 AI 响应以卡片流形式展示（Thinking → Bash → WriteFile → TodoList → Summary）
3. **工具调用可视化**：工具结果默认展示前 200 字符，右下角提供 `+Expand` 按钮展开全部
4. **文件变更对比**：写文件工具在卡片右侧展示新文件内容或 diff 结果
5. **错误重试**：API 调用报错时，在会话上展示错误卡片并提供重试按钮
6. **任务计划追踪**：TodoList 卡片实时展示每个子任务的状态变化

### 1.3 设计方法

采用 **Approach C：可复用 CardWidget 系统**。提取卡片渲染为独立 `CardWidget` 组件，每个卡片自管理标题栏、内容区、右侧面板和底部操作按钮。`Chat` 组件从扁平文本渲染重构为卡片列表渲染。

---

## 2. 架构设计

### 2.1 核心数据模型

#### 2.1.1 Card 结构

```rust
pub struct Card {
    pub id: String,                    // 唯一卡片 ID（ULID 或 UUID）
    pub kind: CardKind,
    pub title: String,
    pub content: String,               // 左侧面板内容（可能为截断版）
    pub full_content: Option<String>,  // 完整内容（截断时存储）
    pub right_content: Option<String>, // 右侧面板内容（diff / 新文件）
    pub state: CardState,
}

pub enum CardKind {
    Thinking,
    ToolUse { name: String },
    ToolResult,
    WriteFile { path: String },
    TodoList { plan_id: String },
    Summary,
    Error,
}

pub enum CardState {
    Animating,   // Thinking... 动画状态
    Collapsed,   // 内容截断到 200 字符
    Expanded,    // 展示完整内容
    Completed,   // 最终完成状态
}
```

#### 2.1.2 Turn 结构

```rust
pub struct Turn {
    pub user_message: String,
    pub cards: Vec<Card>,
    pub is_complete: bool,
}
```

`Turn` 替代原有的 `Message` 列表成为 `Chat` 组件的核心存储单元。每个 Turn 代表一轮完整的用户输入 → AI 响应。

#### 2.1.3 CardAction

```rust
pub enum CardAction {
    Expand(String),    // card_id
    Collapse(String),  // card_id
    Retry(String),     // card_id / turn_index
}
```

### 2.2 组件架构

```
TuiApp
├── Chat (重构)
│   ├── Turn[]
│   │   └── Card[]
│   │       └── CardWidget (新建)
│   │           ├── TitleBar
│   │           ├── ContentArea (左)
│   │           ├── RightPanel (可选，右)
│   │           └── Footer (+Expand / Retry)
│   └── card_hit_areas: Vec<(String, Rect)>
├── Input
├── LeftDrawer
├── RightDrawer
└── StatusBar
```

---

## 3. CardWidget 布局设计

### 3.1 卡片尺寸与布局

每个卡片在终端中占据一个 `Rect`，内部划分为：

```
┌─────────────────────────────────────────────────────────┐
│ 🧠 Thinking...                                          │  ← TitleBar (1行)
├─────────────────────────────────────────────────────────┤
│ 分析项目结构...                                          │  ← ContentArea
│ 需要修改 src/tui/components/chat.rs                      │
│                                           +Expand       │  ← Footer (1行)
└─────────────────────────────────────────────────────────┘
```

**TitleBar**（1 行高）：
- 左侧：图标（🧠/🔧/📤/📝/📋/❌）+ 标题文本
- 样式：主题品牌色（`theme.style_brand()`）+ `Modifier::BOLD`

**ContentArea**（可变高度）：
- 默认左对齐，占卡片宽度的 100%（无右侧面板时）或 60%（有右侧面板时）
- 文本自动换行（`Wrap { trim: true }`）
- 缩进 2 个空格以与 TitleBar 区分

**RightPanel**（可选，与 ContentArea 同高）：
- 占卡片宽度的 40%
- 左侧以竖线分隔（`│` 或 Unicode box drawing）
- 用于展示 diff 或新文件内容
- 内容超出时内部可垂直滚动（独立滚动条）

**Footer**（1 行高）：
- 右对齐
- 展示 `+Expand`（截断时）或 `−Collapse`（展开时）或 `[Retry]`（错误时）
- 样式：主题次级文本色 + 下划线修饰（模拟可点击链接）

### 3.2 鼠标点击检测

`CardWidget` 提供 `handle_click(x, y, rect) -> Option<CardAction>` 方法：

1. 计算 Footer 区域在卡片内的相对位置
2. 判断点击坐标是否落在 Footer 的按钮文本范围内
3. 返回对应的 `CardAction`

`Chat` 组件在 `draw()` 时维护 `card_hit_areas: Vec<(String, Rect)>`，在 `handle_event()` 中遍历匹配鼠标坐标。

---

## 4. 各类型卡片行为

### 4.1 Thinking 卡片

**创建时机**：用户提交消息后，TUI 立即创建（在发送 HTTP 请求之前）。

**动画效果**：
- 状态：`CardState::Animating`
- 标题：`🧠 Thinking` + 动态 dots，每 80ms tick 循环 `.` → `..` → `...`
- 使用现有 `SPINNER_FRAMES` 定时器驱动

**状态转换**：
1. 收到第一个 `SseEvent::Message { content }` 时：
   - 若模型返回了 reasoning 内容 → 卡片转为 `Completed`，展示 reasoning 文本
   - 若无 reasoning → 移除 Thinking 卡片，后续内容直接进入 Summary 或 Tool 卡片
2. 收到 `SseEvent::ToolUse` 时 → Thinking 卡片转为 `Completed`（如有内容）或移除，追加 ToolUse 卡片

### 4.2 ToolUse 卡片

**创建时机**：收到 `SseEvent::ToolUse` 时。

**展示内容**：
- 标题：`🔧 {tool_name}`
- 内容：参数 JSON pretty-print，每行缩进 4 空格

**状态转换**：
- 收到对应的 `SseEvent::ToolResult` 时，卡片 `kind` 从 `ToolUse` 更新为 `ToolResult`，标题变为 `📤 {tool_name} Result`

### 4.3 ToolResult 卡片

**创建时机**：由 ToolUse 卡片转换而来，或直接从 `SseEvent::ToolResult` 创建。

**截断规则**：
- 若 `content.chars().count() > 200`：
  - 展示前 200 字符 + `...`
  - 状态设为 `CardState::Collapsed`
  - Footer 展示 `+Expand`
- 否则：状态直接为 `Completed`，无 Footer 按钮

**展开行为**：
- 点击 `+Expand` → 状态变为 `Expanded`，展示 `full_content`，Footer 变为 `−Collapse`
- 点击 `−Collapse` → 恢复 `Collapsed` 状态

### 4.4 WriteFile 卡片

**创建时机**：检测到 ToolUse / ToolResult 的 `name` 为 `write` 或 `edit`。

**特殊布局**：
- ContentArea（左，60% 宽度）：同 ToolResult 的截断/展开逻辑
- RightPanel（右，40% 宽度）：
  - 若 `is_new_file` → 标题 `New file`，展示完整新内容
  - 若非新文件 → 标题 `Diff`，展示统一 diff 格式（`-` 红色旧行，`+` 绿色新行）

**Diff 生成**：在 `BasicTool::run_write` / `run_edit` 执行时：
1. 写入前读取目标文件原内容（若文件存在）
2. 执行写入操作
3. 使用文本 diff 算法（如 `similar` 库或自定义行级 diff）计算统一 diff
4. 判断 `is_new_file`（原文件不存在或为空）
5. 将 `diff` 和 `is_new_file` 通过扩展的 `SseEvent::ToolResult` 下发到 TUI

### 4.5 TodoList 卡片

**创建时机**：收到 `handle_task_plan` 的 `ToolUse` 时创建，或收到首个 `SseEvent::TaskProgress` 时创建。

**展示内容**：
- 标题：`📋 Task Plan ({n} tasks)`
- 每个任务一行：`{icon} {task_name}`
  - `⏳` Pending
  - `🔵` InProgress
  - `✅` Completed
  - `❌` Failed
- 任务名称后可选展示一行摘要（鼠标悬停时，后续版本实现）

**实时更新**：
- 收到 `SseEvent::TaskProgress` 时，更新对应 `plan_id` 的 TodoList 卡片内部任务状态
- 卡片内容重新渲染，状态图标实时变化

### 4.6 Summary 卡片

**创建时机**：收到第一个非工具相关的 `SseEvent::Message` 文本 token 时创建。

**行为**：
- 标题：`◆ AI`
- 内容随 SSE 流式追加（同现有文本流式逻辑）
- 作为每一轮的最终总结卡片，位于所有工具卡片之后

### 4.7 Error 卡片

**创建时机**：收到 `SseEvent::Error` 时。

**展示内容**：
- 标题：`❌ Error`
- 内容：错误消息文本
- Footer：`[Retry]` 按钮

**重试行为**：
- 点击 `[Retry]` → TUI 发送 `AppEvent::RetryTurn { turn_index }`
- `TuiApp` 处理：使用同一 `session_id` 重新发送用户消息到后端 `/chat` 接口
- 新 Turn 开始，旧 Error Turn 保留在历史中（或标记为过时）

---

## 5. Chat 组件重构

### 5.1 存储结构替换

原有：
```rust
messages: Vec<Message>  // Message { role, content, details, details_expanded }
```

新结构：
```rust
turns: Vec<Turn>        // Turn { user_message, cards, is_complete }
```

**迁移策略**：
- 保留 `add_user_message()` 接口，但内部创建新的 `Turn` 而非 `Message`
- `handle_sse_event()` 不再操作 `Message`，改为操作当前 `Turn` 的 `cards` 列表
- 新增 `create_thinking_card()`、`append_tool_card()`、`update_tool_result()` 等专用方法

### 5.2 渲染流程

```rust
fn draw(&self, frame: &mut Frame, area: Rect, theme: &Theme, is_focused: bool) {
    let inner = block.inner(area);
    let mut current_y = inner.y;
    self.card_hit_areas.clear();

    for turn in &self.turns {
        // 1. 渲染用户消息
        let user_lines = turn.user_message.lines().count();
        render_user_message(frame, turn.user_message, Rect { y: current_y, .. }, theme);
        current_y += user_lines as u16 + 1;

        // 2. 渲染卡片
        for card in &turn.cards {
            let card_height = card.calculate_height(inner.width);
            let card_area = Rect { y: current_y, height: card_height, ..inner };
            
            CardWidget::new(card).draw(frame, card_area, theme);
            self.card_hit_areas.push((card.id.clone(), card_area));
            
            current_y += card_height;
        }
    }

    // 3. 若正在生成，底部显示 spinner（可选，Thinking 卡片已覆盖此场景）
}
```

### 5.3 滚动逻辑

- `scroll_offset` 从"消息索引"改为"内容行偏移"
- 每次卡片内容变化（新增卡片、展开/折叠）时，若用户未手动上滚，自动滚动到底部
- 手动滚动后，保持当前位置直到用户滚动回底部

### 5.4 事件处理

```rust
fn handle_event(&mut self, event: &Event, _focus: bool) -> Option<AppEvent> {
    match event {
        Event::Mouse(mouse) => self.handle_mouse_event(mouse),
        Event::Key(key) => self.handle_key_event(key),
        _ => None,
    }
}
```

**鼠标事件**：
- 遍历 `card_hit_areas`，找到包含鼠标坐标的卡片
- 调用 `CardWidget::handle_click()` 判断是否点击了 Footer 按钮
- 返回 `AppEvent::CardAction(action)`

**键盘事件**：
- `PageUp` / `Ctrl+↑`：向上滚动一页
- `PageDown` / `Ctrl+↓`：向下滚动一页
- `Enter`：保留为"折叠/展开最后一张 Assistant 卡片"的快捷键（兼容旧习惯）

---

## 6. 后端 SSE 协议扩展

### 6.1 现有事件

| 事件 | 现有？ | 用途 |
|------|--------|------|
| `Message` | ✅ | 流式文本 token |
| `MessageDetails` | ✅ | 批量发送详情块（回合结束时） |
| `Error` | ✅ | 错误信息 |
| `Done` | ✅ | 回合结束 |
| `Usage` | ✅ | Token 使用量 |
| `ToolUse` | ⚠️ 定义存在但未发送 | 工具调用通知 |
| `ToolResult` | ⚠️ 定义存在但未发送 | 工具结果通知 |

### 6.2 新增/激活事件

#### ToolUse 事件（激活现有定义）

在 `agent_loop` 检测到 `FinishReason::ToolUse` 后、执行工具前发送：

```rust
for block in &content_blocks {
    if let Part::ToolUse { id, name, arguments } = block {
        if let Some(ref mut cb) = on_tool_event {
            cb(SseEvent::ToolUse {
                id: id.clone(),
                name: name.clone(),
                arguments: arguments.clone(),
            });
        }
    }
}
```

#### ToolResult 事件（激活 + 扩展）

扩展 `SseEvent::ToolResult` 以支持 diff 数据：

```rust
#[serde(rename = "tool_result")]
ToolResult {
    tool_use_id: String,
    content: String,
    diff: Option<String>,       // 新增：统一 diff 文本
    is_new_file: bool,          // 新增：是否为新文件
},
```

> **注意**：向现有 enum variant 添加字段是 Rust 中的破坏性变更，需要更新所有 match 该 variant 的代码。项目中目前没有任何代码匹配 `SseEvent::ToolUse` 或 `SseEvent::ToolResult`（grep 确认无匹配），因此安全。`chat_api.rs` 和 `tui/components/chat.rs` 中需要新增处理逻辑。

在 `execute_tool_calls` 的每个工具 future 完成时发送：

```rust
async move {
    let (content, is_error, diff, is_new_file) = 
        execute_single_tool_call_with_diff(&id, &name, &arguments).await;
    
    if let Some(ref mut cb) = on_tool_event {
        cb(SseEvent::ToolResult {
            tool_use_id: id.clone(),
            content: content.clone(),
            diff,
            is_new_file,
        });
    }
    
    Part::ToolResult { tool_call_id: id, content, is_error }
}
```

#### TaskProgress 事件（新增）

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskProgressItem {
    pub id: String,
    pub name: String,
    pub status: TaskStatus,  // Pending | InProgress | Completed | Failed
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SseEvent {
    // ... 现有事件 ...
    
    #[serde(rename = "task_progress")]
    TaskProgress {
        plan_id: String,
        tasks: Vec<TaskProgressItem>,
    },
}
```

在 `execute_handle_task_plan` 的 `on_progress` 回调中发送：

```rust
let sse_sender_for_progress = sse_sender.clone();
let mut on_progress = move |plan: &TaskPlan| {
    let items: Vec<TaskProgressItem> = plan
        .tasks
        .iter()
        .map(|t| TaskProgressItem {
            id: t.id.clone(),
            name: t.name.clone(),
            status: t.status.clone(),
        })
        .collect();
    
    let _ = sse_sender_for_progress.try_send(SseEvent::TaskProgress {
        plan_id: plan_id.clone(),
        tasks: items,
    });
};
```

### 6.3 回调签名调整

`run_one_turn` 和 `agent_loop` 需要新增 `on_tool_event` 回调参数：

```rust
pub async fn agent_loop<C: AIClient + ?Sized>(
    client: &C,
    state: &mut LoopState,
    on_text: &mut Option<Box<dyn FnMut(&str) + Send>>,
    on_tool_event: &mut Option<Box<dyn FnMut(SseEvent) + Send>>,  // 新增
) -> Result<()> {
    while run_one_turn(client, state, on_text, on_tool_event).await? {}
    Ok(())
}
```

`chat_api.rs` 中的 `run_agent_chat` 需要创建 `on_tool_event` 回调并通过 `sse_sender` 发送事件。

### 6.4 消息历史截断兼容

现有的 `MessageDetails` 批量事件仍保留发送（在 `send_last_assistant_details` 中），用于：
1. 向后兼容其他客户端
2. 作为回合结束时的状态同步校验

TUI 优先使用实时事件（`ToolUse`/`ToolResult`）构建卡片，回合结束时的 `MessageDetails` 用于校验/补全缺失状态。

---

## 7. 错误处理与重试

### 7.1 错误展示

- API 调用异常（网络超时、鉴权失败、模型错误）→ `SseEvent::Error` → TUI 创建 Error 卡片
- 工具执行异常（权限拒绝、路径逃逸）→ 在 `ToolResult` 的 `content` 中展示，卡片标题旁显示 `❌` 标记

### 7.2 重试机制

1. **TUI 层**：Error 卡片的 `[Retry]` 按钮点击 → `AppEvent::RetryTurn { turn_index }`
2. **App 层**：`TuiApp::handle_app_event()` 收到 RetryTurn → 调用 `self.start_chat_stream(user_msg, session_id)` 重新发送请求
3. **后端层**：复用现有会话，继续 `agent_loop`
4. **UI 层**：新 Turn 开始，旧 Error Turn 保留但可标记为 `is_stale`（视觉上置灰）

---

## 8. 测试策略

### 8.1 单元测试

| 测试项 | 覆盖内容 |
|--------|----------|
| `CardWidget::calculate_height()` | 不同内容长度、有无右侧面板、展开/折叠状态的高度计算 |
| `CardWidget::handle_click()` | 点击 Footer 按钮区域、点击内容区域、点击边界外 |
| `Chat::handle_sse_event()` | Message → Summary 卡片、ToolUse → ToolResult 转换、TaskProgress 更新 |
| `Chat::scroll_behavior()` | 新增卡片自动滚底、手动滚动后保持位置 |
| `execute_tool_calls_with_events()` | 验证每个工具完成时正确触发 ToolResult 事件 |

### 8.2 集成测试

- 使用 `wiremock` 模拟 SSE 流，验证完整回合的卡片序列（Thinking → ToolUse → ToolResult → Summary）
- 验证鼠标点击 Expand/Collapse 按钮后卡片状态变化
- 验证重试流程的端到端行为

---

## 9. 实现顺序建议

1. **Phase 1：CardWidget 基础**
   - 创建 `CardWidget` 组件，实现基础布局（TitleBar + ContentArea）
   - 实现 `calculate_height` 和 `draw`
   - 单元测试

2. **Phase 2：Chat 组件重构**
   - 引入 `Turn` / `Card` 数据模型
   - 重构 `Chat::draw()` 为卡片列表渲染
   - 实现鼠标点击检测框架
   - 保持现有 SSE 事件兼容（使用 MessageDetails 构建初始卡片）

3. **Phase 3：实时工具事件**
   - 修改 `agent_loop` / `run_one_turn` 添加 `on_tool_event` 回调
   - 在 `chat_api.rs` 中发送 `ToolUse` / `ToolResult` 事件
   - TUI 处理实时事件动态创建/更新卡片

4. **Phase 4：特殊卡片**
   - Thinking 占位卡片 + 动画
   - ToolResult 截断 + Expand/Collapse
   - WriteFile diff 右侧面板
   - Error 卡片 + Retry

5. **Phase 5：TodoList**
   - 新增 `TaskProgress` SSE 事件
   - 修改 `execute_handle_task_plan` 发送进度事件
   - 实现 TodoListCard 渲染

---

## 10. 风险与注意事项

1. **性能**：大量卡片同时渲染可能影响帧率。每个卡片的 `calculate_height` 需要在 draw 时快速完成。考虑缓存高度计算结果。
2. **状态同步**：实时事件与回合结束时的 `MessageDetails` 可能存在时序冲突。TUI 应以实时事件为准，`MessageDetails` 仅用于补全。
3. **鼠标支持**：需要确保 `crossterm` 的鼠标事件捕获已启用（当前 `src/tui/mod.rs` 已启用 `EnableMouseCapture`）。
4. **Diff 计算**：大文件的 diff 可能非常长。需要对 diff 输出也做截断，或在右侧面板内实现独立滚动。
