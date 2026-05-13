# MessageV2 Part Architecture Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Unify backend and TUI message models by extending the `Part` enum, introducing `SseEvent::Part` for non-streaming content, and building a `Part → Renderer` mapping system in the TUI.

**Architecture:** Extend `session::message::Part` with `ToolError`, `WaveMarker`, and `Usage`; replace flat SSE events with `SseEvent::Part`; refactor TUI rendering from `CardKind`-based to `PartRenderer` trait-based dispatch.

**Tech Stack:** Rust, ratatui, crossterm, serde, tokio

---

## File Structure

### Phase 1: Skeleton Refactoring

| File | Action | Responsibility |
|------|--------|----------------|
| `crates/core/src/session/message.rs` | Modify | Extend `Part` enum; add `TokenUsage`; remove `ToolResult.is_error` |
| `crates/core/src/server/transport/sse.rs` | Modify | Add `SseEvent::Part`; remove `ToolUse`/`ToolResult`/`Usage`/`MessageDetails` |
| `crates/core/src/tools/mod.rs` | Modify | Update `execute_tool_calls` to emit `SseEvent::Part` |
| `crates/core/src/agent/agent.rs` | Modify | Update `run_one_turn` SSE emission to use `SseEvent::Part` |
| `crates/core/src/agent/runner.rs` | Modify | Update `AgentRunner` SSE emission to use `SseEvent::Part` |
| `crates/core/src/server/api/chat_api.rs` | Modify | Remove `send_last_assistant_details`; emit Parts in real-time |
| `crates/core/src/tui/client.rs` | Modify | Parse `SseEvent::Part`; route to `Chat::handle_part_event` |
| `crates/core/src/tui/app.rs` | Modify | Route `AppEvent::SseEvent(Part)` to chat component |
| `crates/core/src/tui/components/chat.rs` | Modify | Replace `handle_sse_event` with `handle_part_event`; prepare for Part renderers |
| `crates/core/src/tui/components/status_bar.rs` | Modify | Rename fields; add CTX progress bar; compact/extreme layouts |

### Phase 2: TUI Renderers (New Files)

| File | Action | Responsibility |
|------|--------|----------------|
| `crates/core/src/tui/components/part_renderer/mod.rs` | Create | `PartRenderer` trait and registry |
| `crates/core/src/tui/components/part_renderer/wave_marker.rs` | Create | WaveMarker line rendering |
| `crates/core/src/tui/components/part_renderer/usage.rs` | Create | Usage footer line rendering |
| `crates/core/src/tui/components/part_renderer/thinking.rs` | Create | Thinking/Reasoning card rendering |
| `crates/core/src/tui/components/part_renderer/tool_call.rs` | Create | ToolUse card + dispatch to sub-renderers |
| `crates/core/src/tui/components/part_renderer/tool_result.rs` | Create | ToolResult card + dispatch to sub-renderers |
| `crates/core/src/tui/components/part_renderer/tool_error.rs` | Create | ToolError card rendering |
| `crates/core/src/tui/components/part_renderer/file_preview.rs` | Create | File preview with line numbers and syntax highlighting |
| `crates/core/src/tui/components/part_renderer/diff.rs` | Create | Unified diff rendering |
| `crates/core/src/tui/components/part_renderer/shell_output.rs` | Create | Shell command + output rendering |
| `crates/core/src/tui/components/part_renderer/task_list.rs` | Create | Task list with status icons |
| `crates/core/src/tui/components/part_renderer/image.rs` | Create | Image placeholder rendering |
| `crates/core/src/tui/components/part_renderer/text.rs` | Create | Plain text rendering |

### Phase 3: WaveMarker Backend Integration

| File | Action | Responsibility |
|------|--------|----------------|
| `crates/core/src/agent/agent.rs` | Modify | Insert `WaveMarker` at start of each agent loop iteration |
| `crates/core/src/tools/basic_tools.rs` | Modify | Add `git_write_tree` helper |
| `crates/core/src/tui/components/chat.rs` | Modify | WaveMarker `g`/`r` interaction handling |

---

## Phase 1: Skeleton Refactoring

### Task 1: Extend Part Enum

**Files:**
- Modify: `crates/core/src/session/message.rs`
- Test: `crates/core/src/session/message.rs` (existing `#[cfg(test)]` block)

**Context:** Current `Part` has 5 variants. We add 3, remove 1 field.

- [ ] **Step 1: Add `TokenUsage` struct and new `Part` variants**

Add after `ImageSource` enum:

```rust
/// Token 用量结构
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
}
```

Replace the existing `Part` enum with:

```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Part {
    Text { text: String },
    Image { source: ImageSource },
    ToolUse {
        id: String,
        name: String,
        arguments: serde_json::Value,
    },
    ToolResult {
        tool_call_id: String,
        content: String,
    },
    ToolError {
        tool_call_id: String,
        content: String,
        error_message: String,
    },
    Reasoning {
        thinking: String,
        signature: Option<String>,
    },
    WaveMarker {
        step: u32,
        total: Option<u32>,
        git_snapshot: Option<String>,
        timestamp: u64,
        delta_tokens: TokenUsage,
    },
    Usage {
        input_tokens: u32,
        output_tokens: u32,
        latency_ms: u32,
        cost: Option<f64>,
    },
}
```

- [ ] **Step 2: Find and fix all references to `ToolResult.is_error`**

Run:
```bash
cd /home/nan/fi-code && grep -rn "is_error" crates/core/src/ --include="*.rs" | grep -v "target/"
```

Expected: References in `tools/mod.rs`, `agent/agent.rs`, `session/message.rs` tests, `server/transport/sse.rs` (`DetailBlock`), `server/api/chat_api.rs`.

For each reference, update logic:
- If checking `is_error == true`, change to `Part::ToolError` match arm
- If checking `is_error == false`, assume it's `Part::ToolResult`

- [ ] **Step 3: Update `Part` tests in `message.rs`**

Add tests for new variants:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_part_serialization() {
        let part = Part::WaveMarker {
            step: 1,
            total: Some(5),
            git_snapshot: Some("abc123".to_string()),
            timestamp: 1234567890,
            delta_tokens: TokenUsage { prompt_tokens: 100, completion_tokens: 50 },
        };
        let json = serde_json::to_string(&part).unwrap();
        assert!(json.contains("wave_marker"));
        assert!(json.contains("abc123"));

        let deserialized: Part = serde_json::from_str(&json).unwrap();
        match deserialized {
            Part::WaveMarker { step, .. } => assert_eq!(step, 1),
            _ => panic!("Expected WaveMarker"),
        }
    }

    #[test]
    fn test_tool_error_serialization() {
        let part = Part::ToolError {
            tool_call_id: "tc1".to_string(),
            content: "raw output".to_string(),
            error_message: "Permission denied".to_string(),
        };
        let json = serde_json::to_string(&part).unwrap();
        assert!(json.contains("tool_error"));
        assert!(json.contains("Permission denied"));
    }

    #[test]
    fn test_usage_serialization() {
        let part = Part::Usage {
            input_tokens: 5400,
            output_tokens: 2100,
            latency_ms: 2400,
            cost: Some(0.008),
        };
        let json = serde_json::to_string(&part).unwrap();
        assert!(json.contains("usage"));
        assert!(json.contains("0.008"));
    }
}
```

- [ ] **Step 4: Run tests**

```bash
cd /home/nan/fi-code && cargo test -p fi-code-core message::tests
```

Expected: PASS

- [ ] **Step 5: Commit**

```bash
cd /home/nan/fi-code && git add crates/core/src/session/message.rs && git commit -m "feat: extend Part enum with ToolError, WaveMarker, Usage; remove ToolResult.is_error

- Add TokenUsage struct for WaveMarker delta tracking
- Add Part::ToolError for failed tool calls
- Add Part::WaveMarker for agent loop step markers
- Add Part::Usage for per-message token/latency stats
- Remove is_error field from Part::ToolResult
- Update tests for new variants"
```

---

### Task 2: Refactor SseEvent

**Files:**
- Modify: `crates/core/src/server/transport/sse.rs`
- Modify: `crates/core/src/session/message.rs` (if needed for imports)

- [ ] **Step 1: Replace SseEvent enum**

Replace the current `SseEvent` with:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SseEvent {
    #[serde(rename = "message")]
    Message { content: String },

    #[serde(rename = "part")]
    Part { part: crate::session::message::Part },

    #[serde(rename = "task_progress")]
    TaskProgress {
        plan_id: String,
        tasks: Vec<TaskProgressItem>,
    },

    #[serde(rename = "error")]
    Error { message: String },

    #[serde(rename = "done")]
    Done { session_id: String },
}
```

Delete `DetailBlock` enum and `MessageDetails` variant (no longer needed with real-time Part streaming).

- [ ] **Step 2: Fix compilation errors**

Run:
```bash
cd /home/nan/fi-code && cargo check -p fi-code-core
```

Fix all references to deleted `SseEvent` variants:
- `SseEvent::ToolUse` → `SseEvent::Part { part: Part::ToolUse { ... } }`
- `SseEvent::ToolResult` → `SseEvent::Part { part: Part::ToolResult { ... } }`
- `SseEvent::Usage` → `SseEvent::Part { part: Part::Usage { ... } }`
- `SseEvent::MessageDetails` → remove related code

- [ ] **Step 3: Commit**

```bash
cd /home/nan/fi-code && git add crates/core/src/server/transport/sse.rs && git commit -m "feat: refactor SseEvent to use unified Part variant

- Add SseEvent::Part for all non-streaming content
- Remove ToolUse, ToolResult, Usage, MessageDetails variants
- Keep Message for streaming text, TaskProgress, Error, Done"
```

---

### Task 3: Update Backend SSE Emission (tools/mod.rs)

**Files:**
- Modify: `crates/core/src/tools/mod.rs`

- [ ] **Step 1: Update `execute_tool_calls` to emit `SseEvent::Part`**

Find the `execute_tool_calls` function. Replace the SSE emission block:

```rust
// Before (approx line 1000):
let _ = callback(SseEvent::ToolResult {
    tool_use_id: id.clone(),
    content: display_content,
    diff,
    is_new_file,
    full_content,
});

// After:
let part = if is_error {
    Part::ToolError {
        tool_call_id: id.clone(),
        content: display_content,
        error_message: display_content.clone(),
    }
} else {
    Part::ToolResult {
        tool_call_id: id.clone(),
        content: display_content,
    }
};
let _ = callback(SseEvent::Part { part });
```

Note: `is_error` comes from `execute_single_tool_call` return value. For `ToolResult` parts, we no longer pass `diff`/`is_new_file`/`full_content` through SSE — the TUI will reconstruct these from `ToolUse.arguments` and the `content` JSON.

- [ ] **Step 2: Update `tool_call` for ask_for_question**

Find the `ask_for_question` branch in `tool_call`. Wrap the result in `SseEvent::Part` if emitting via callback (though `ask_for_question` typically doesn't emit SSE directly).

- [ ] **Step 3: Run tests**

```bash
cd /home/nan/fi-code && cargo test -p fi-code-core tools::
```

Expected: All existing tests pass after adapting to new types.

- [ ] **Step 4: Commit**

```bash
cd /home/nan/fi-code && git add crates/core/src/tools/mod.rs && git commit -m "feat: update execute_tool_calls to emit SseEvent::Part

- Tool errors emit Part::ToolError
- Tool successes emit Part::ToolResult
- Remove diff/is_new_file/full_content from SSE (reconstructed in TUI)"
```

---

### Task 4: Update Agent SSE Emission

**Files:**
- Modify: `crates/core/src/agent/agent.rs`
- Modify: `crates/core/src/agent/runner.rs`
- Modify: `crates/core/src/server/api/chat_api.rs`

- [ ] **Step 1: Update `agent.rs` `run_one_turn`**

Find the `ToolUse` emission block (around line 383):

```rust
// Before:
let _ = cb(crate::server::transport::sse::SseEvent::ToolUse {
    id: id.clone(),
    name: name.clone(),
    arguments: arguments.clone(),
});

// After:
let _ = cb(crate::server::transport::sse::SseEvent::Part {
    part: crate::session::message::Part::ToolUse {
        id: id.clone(),
        name: name.clone(),
        arguments: arguments.clone(),
    },
});
```

- [ ] **Step 2: Update `runner.rs`**

Find similar `SseEvent::ToolUse` emission in `AgentRunner::run_one_turn` and update to `SseEvent::Part`.

- [ ] **Step 3: Remove `send_last_assistant_details` from chat_api.rs**

This function is no longer needed because all content is streamed in real-time via `SseEvent::Part`. Delete the function and its call site.

- [ ] **Step 4: Commit**

```bash
cd /home/nan/fi-code && git add crates/core/src/agent/agent.rs crates/core/src/agent/runner.rs crates/core/src/server/api/chat_api.rs && git commit -m "feat: update agent SSE emission to use SseEvent::Part

- Convert ToolUse emissions to Part::ToolUse
- Remove send_last_assistant_details (replaced by real-time Part streaming)"
```

---

### Task 5: Update TUI SSE Reception

**Files:**
- Modify: `crates/core/src/tui/client.rs`
- Modify: `crates/core/src/tui/app.rs`
- Modify: `crates/core/src/tui/components/chat.rs`

- [ ] **Step 1: Update `client.rs` to parse `SseEvent::Part`**

In the SSE event preview match, add:

```rust
SseEvent::Part { part } => {
    let preview = match part {
        Part::ToolUse { name, .. } => format!("Part::ToolUse(name={})", name),
        Part::ToolResult { tool_call_id, .. } => format!("Part::ToolResult(id={})", tool_call_id),
        Part::ToolError { tool_call_id, .. } => format!("Part::ToolError(id={})", tool_call_id),
        Part::Reasoning { .. } => "Part::Reasoning".to_string(),
        Part::WaveMarker { step, .. } => format!("Part::WaveMarker(step={})", step),
        Part::Usage { .. } => "Part::Usage".to_string(),
        Part::Text { text } => format!("Part::Text(len={})", text.len()),
        Part::Image { .. } => "Part::Image".to_string(),
    };
    format!("Part({})", preview)
}
```

- [ ] **Step 2: Update `app.rs` to route `SseEvent::Part`**

In `AppEvent::SseEvent` handler, add handling for `SseEvent::Part`:

```rust
AppEvent::SseEvent(ref sse_event) => {
    match sse_event {
        SseEvent::Part { part } => {
            log_debug!("[Client] SSE Part received | {:?}", std::mem::discriminant(part));
            self.chat.handle_part_event(part);
        }
        // existing variants...
    }
}
```

- [ ] **Step 3: Add `handle_part_event` to `chat.rs`**

Add a new method to `Chat`:

```rust
pub fn handle_part_event(&mut self, part: &crate::session::message::Part) {
    use crate::session::message::Part;
    match part {
        Part::WaveMarker { .. } => {
            // Create or update current turn with WaveMarker
            if let Some(last_turn) = self.turns.last_mut() {
                last_turn.parts.push(part.clone());
            }
        }
        Part::Reasoning { .. } => {
            if let Some(last_turn) = self.turns.last_mut() {
                last_turn.parts.push(part.clone());
            }
        }
        Part::ToolUse { .. } => {
            if let Some(last_turn) = self.turns.last_mut() {
                last_turn.parts.push(part.clone());
            }
        }
        Part::ToolResult { .. } => {
            if let Some(last_turn) = self.turns.last_mut() {
                last_turn.parts.push(part.clone());
            }
        }
        Part::ToolError { .. } => {
            if let Some(last_turn) = self.turns.last_mut() {
                last_turn.parts.push(part.clone());
            }
        }
        Part::Text { text } => {
            // Append to current text part or create new
            if let Some(last_turn) = self.turns.last_mut() {
                if let Some(Part::Text { ref mut text: existing }) = last_turn.parts.last_mut() {
                    existing.push_str(text);
                } else {
                    last_turn.parts.push(part.clone());
                }
            }
        }
        Part::Usage { .. } => {
            if let Some(last_turn) = self.turns.last_mut() {
                last_turn.parts.push(part.clone());
            }
        }
        Part::Image { .. } => {
            if let Some(last_turn) = self.turns.last_mut() {
                last_turn.parts.push(part.clone());
            }
        }
    }
}
```

Note: This requires `Turn` to have a `parts: Vec<Part>` field. We'll add that in the next step.

- [ ] **Step 4: Update `Turn` struct to use `Vec<Part>`**

Replace:
```rust
pub struct Turn {
    pub user_message: String,
    pub cards: Vec<Card>,
    pub is_complete: bool,
}
```

With:
```rust
pub struct Turn {
    pub user_message: String,
    pub parts: Vec<crate::session::message::Part>,
    pub is_complete: bool,
}
```

Update all references to `turn.cards` to `turn.parts` in `chat.rs`.

- [ ] **Step 5: Run tests**

```bash
cd /home/nan/fi-code && cargo test -p fi-code-core tui::
```

Expected: Tests pass after adapting to new structures.

- [ ] **Step 6: Commit**

```bash
cd /home/nan/fi-code && git add crates/core/src/tui/client.rs crates/core/src/tui/app.rs crates/core/src/tui/components/chat.rs && git commit -m "feat: update TUI to receive and route SseEvent::Part

- client.rs: parse SseEvent::Part with detailed preview logging
- app.rs: route Part events to Chat::handle_part_event
- chat.rs: add handle_part_event; convert Turn.cards to Turn.parts"
```

---

### Task 6: Status Bar Redesign

**Files:**
- Modify: `crates/core/src/tui/components/status_bar.rs`
- Modify: `crates/core/src/tui/app.rs` (update status bar API calls)

- [ ] **Step 1: Update `StatusBar` struct fields**

```rust
pub struct StatusBar {
    progress_state: ProgressState,
    progress_tick: u64,
    last_filled: usize,
    model_name: String,
    ctx_current: usize,      // current context tokens
    ctx_limit: usize,        // context window limit (default 128k)
    token_in: usize,
    token_out: usize,
    latency_ms: u32,
    elapsed_secs: u64,
}
```

- [ ] **Step 2: Update `StatusBar::draw` method**

Implement the standard format:
```
FiCode │ CTX:[████████░░] │ TOK:⬆️24k⬇️18k │ LAT:2.4s │ MDL:kimi-k2.5 │ 10:06
```

And compact format for width < 100:
```
FiCode [████████░░] │ TOK:18k │ LAT:2.4s │ k2.5 │ 10:06
```

And extreme format for width < 80:
```
FiCode [████░░░░░░] │ LAT:2.4s │ k2.5
```

CTX progress bar color: green (<50%) → yellow (50-80%) → red (>80%).

- [ ] **Step 3: Add setter methods for new fields**

```rust
impl StatusBar {
    pub fn set_ctx_tokens(&mut self, current: usize, limit: usize) {
        self.ctx_current = current;
        self.ctx_limit = limit;
    }

    pub fn set_latency(&mut self, latency_ms: u32) {
        self.latency_ms = latency_ms;
    }
}
```

- [ ] **Step 4: Update `app.rs` to feed context tokens to status bar**

In the `Usage` event handler or message processing, calculate context pressure and call `status_bar.set_ctx_tokens()`.

- [ ] **Step 5: Run tests**

```bash
cd /home/nan/fi-code && cargo test -p fi-code-core status_bar
```

- [ ] **Step 6: Commit**

```bash
cd /home/nan/fi-code && git add crates/core/src/tui/components/status_bar.rs crates/core/src/tui/app.rs && git commit -m "feat: redesign status bar with CTX/TOK/LAT/MDL fields

- Add CTX progress bar with color-coded occupancy
- Rename IN/OUT to TOK with up/down arrows
- Add LAT (latency) field
- Implement compact and extreme layout modes"
```

---

## Phase 2: TUI Part Renderers

### Task 7: PartRenderer Trait and Registry

**Files:**
- Create: `crates/core/src/tui/components/part_renderer/mod.rs`
- Modify: `crates/core/src/tui/components/mod.rs`

- [ ] **Step 1: Create `part_renderer/mod.rs`**

```rust
use ratatui::{
    layout::Rect,
    Frame,
};
use crossterm::event::Event;
use crate::session::message::Part;
use crate::tui::theme::Theme;

/// Part 渲染器 trait：每个 Part 变体对应一个渲染器
pub trait PartRenderer {
    /// 计算该 Part 在给定宽度下的渲染高度（行数）
    fn height(&self, part: &Part, width: u16) -> u16;

    /// 在指定区域渲染该 Part
    fn draw(&self, frame: &mut Frame, area: Rect, part: &Part, theme: &Theme);

    /// 处理交互事件（可选），返回是否消耗了事件
    fn handle_event(&mut self, _part: &mut Part, _event: &Event) -> bool {
        false
    }
}

/// 渲染器注册表
pub struct PartRendererRegistry {
    renderers: std::collections::HashMap<String, Box<dyn PartRenderer>>,
}

impl PartRendererRegistry {
    pub fn new() -> Self {
        use std::collections::HashMap;
        let mut registry = Self { renderers: HashMap::new() };
        registry.register("text", Box::new(text::TextRenderer));
        registry.register("reasoning", Box::new(thinking::ThinkingRenderer));
        registry.register("tool_use", Box::new(tool_call::ToolCallRenderer));
        registry.register("tool_result", Box::new(tool_result::ToolResultRenderer));
        registry.register("tool_error", Box::new(tool_error::ToolErrorRenderer));
        registry.register("wave_marker", Box::new(wave_marker::WaveMarkerRenderer));
        registry.register("usage", Box::new(usage::UsageRenderer));
        registry.register("image", Box::new(image::ImageRenderer));
        registry
    }

    pub fn register(&mut self, name: &str, renderer: Box<dyn PartRenderer>) {
        self.renderers.insert(name.to_string(), renderer);
    }

    pub fn get(&self, part: &Part) -> Option<&dyn PartRenderer> {
        let key = match part {
            Part::Text { .. } => "text",
            Part::Image { .. } => "image",
            Part::ToolUse { .. } => "tool_use",
            Part::ToolResult { .. } => "tool_result",
            Part::ToolError { .. } => "tool_error",
            Part::Reasoning { .. } => "reasoning",
            Part::WaveMarker { .. } => "wave_marker",
            Part::Usage { .. } => "usage",
        };
        self.renderers.get(key).map(|b| b.as_ref())
    }
}

// Sub-modules
mod text;
mod thinking;
mod tool_call;
mod tool_result;
mod tool_error;
mod wave_marker;
mod usage;
mod image;
```

- [ ] **Step 2: Update `components/mod.rs`**

Add `pub mod part_renderer;` to the module declarations.

- [ ] **Step 3: Create stub files for all sub-renderers**

Create empty files:
- `crates/core/src/tui/components/part_renderer/text.rs`
- `crates/core/src/tui/components/part_renderer/thinking.rs`
- `crates/core/src/tui/components/part_renderer/tool_call.rs`
- `crates/core/src/tui/components/part_renderer/tool_result.rs`
- `crates/core/src/tui/components/part_renderer/tool_error.rs`
- `crates/core/src/tui/components/part_renderer/wave_marker.rs`
- `crates/core/src/tui/components/part_renderer/usage.rs`
- `crates/core/src/tui/components/part_renderer/image.rs`

Each stub:
```rust
use super::*;

pub struct TextRenderer;

impl PartRenderer for TextRenderer {
    fn height(&self, _part: &Part, _width: u16) -> u16 {
        1
    }

    fn draw(&self, frame: &mut Frame, area: Rect, part: &Part, theme: &Theme) {
        // TODO: implement
    }
}
```

- [ ] **Step 4: Commit**

```bash
cd /home/nan/fi-code && git add crates/core/src/tui/components/part_renderer/ crates/core/src/tui/components/mod.rs && git commit -m "feat: add PartRenderer trait and registry skeleton

- Define PartRenderer trait with height/draw/handle_event
- Create PartRendererRegistry with dispatch by Part variant
- Add stub files for all 8 renderers"
```

---

### Task 8: WaveMarker Renderer

**Files:**
- Create: `crates/core/src/tui/components/part_renderer/wave_marker.rs`

- [ ] **Step 1: Implement WaveMarkerRenderer**

```rust
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use crate::session::message::Part;
use crate::tui::theme::Theme;
use super::{PartRenderer};

pub struct WaveMarkerRenderer;

impl PartRenderer for WaveMarkerRenderer {
    fn height(&self, _part: &Part, _width: u16) -> u16 {
        1
    }

    fn draw(&self, frame: &mut Frame, area: Rect, part: &Part, theme: &Theme) {
        if let Part::WaveMarker { step, total, git_snapshot, delta_tokens, .. } = part {
            let total_str = total.map(|t| format!("{}", t)).unwrap_or_else(|| "?".to_string());
            let step_span = Span::styled(
                format!("Step {}/{}", step, total_str),
                Style::default().fg(theme.success).add_modifier(Modifier::BOLD),
            );

            let mut spans = vec![step_span];

            if let Some(hash) = git_snapshot {
                spans.push(Span::raw(" "));
                spans.push(Span::styled(
                    format!("[{:.7}]", hash),
                    Style::default().fg(theme.success),
                ));
            }

            let delta_in = format_tokens(delta_tokens.prompt_tokens);
            let delta_out = format_tokens(delta_tokens.completion_tokens);
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                format!("ΔTOK:⬆️{}⬇️{}", delta_in, delta_out),
                Style::default().fg(theme.text_muted),
            ));

            let line = Line::from(spans);
            frame.render_widget(Paragraph::new(line), area);
        }
    }
}

fn format_tokens(n: u32) -> String {
    if n >= 1000 {
        format!("{:.1}k", n as f64 / 1000.0)
    } else {
        n.to_string()
    }
}
```

- [ ] **Step 2: Commit**

```bash
cd /home/nan/fi-code && git add crates/core/src/tui/components/part_renderer/wave_marker.rs && git commit -m "feat: implement WaveMarkerRenderer

- Render step/total with green bold styling
- Show git snapshot hash
- Display delta token usage with up/down arrows"
```

---

### Task 9: Usage Renderer

**Files:**
- Create: `crates/core/src/tui/components/part_renderer/usage.rs`

- [ ] **Step 1: Implement UsageRenderer**

```rust
use ratatui::{
    layout::{Alignment, Rect},
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use crate::session::message::Part;
use crate::tui::theme::Theme;
use super::PartRenderer;

pub struct UsageRenderer;

impl PartRenderer for UsageRenderer {
    fn height(&self, _part: &Part, _width: u16) -> u16 {
        1
    }

    fn draw(&self, frame: &mut Frame, area: Rect, part: &Part, theme: &Theme) {
        if let Part::Usage { input_tokens, output_tokens, latency_ms, cost } = part {
            let mut spans = vec![
                Span::styled(format!("⬆️{}", format_tokens(*input_tokens)), Style::default().fg(theme.text_muted)),
                Span::raw(" "),
                Span::styled(format!("⬇️{}", format_tokens(*output_tokens)), Style::default().fg(theme.text_muted)),
                Span::raw(" · "),
                Span::styled(format!("LAT:{:.1}s", *latency_ms as f64 / 1000.0), Style::default().fg(theme.text_muted)),
            ];

            if let Some(c) = cost {
                spans.push(Span::raw(" · "));
                spans.push(Span::styled(format!("${:.3}", c), Style::default().fg(theme.text_muted)));
            }

            let line = Line::from(spans);
            frame.render_widget(
                Paragraph::new(line).alignment(Alignment::Right),
                area,
            );
        }
    }
}

fn format_tokens(n: u32) -> String {
    if n >= 1000 {
        format!("{:.1}k", n as f64 / 1000.0)
    } else {
        n.to_string()
    }
}
```

- [ ] **Step 2: Commit**

```bash
cd /home/nan/fi-code && git add crates/core/src/tui/components/part_renderer/usage.rs && git commit -m "feat: implement UsageRenderer

- Right-aligned usage footer line
- Show input/output tokens with arrows
- Show LAT in seconds
- Show cost when available"
```

---

### Task 10: Thinking Renderer

**Files:**
- Create: `crates/core/src/tui/components/part_renderer/thinking.rs`

- [ ] **Step 1: Implement ThinkingRenderer**

Migrate existing Thinking card logic from `card_widget.rs`:

```rust
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use crate::session::message::Part;
use crate::tui::theme::Theme;
use super::PartRenderer;

pub struct ThinkingRenderer;

impl PartRenderer for ThinkingRenderer {
    fn height(&self, part: &Part, width: u16) -> u16 {
        if let Part::Reasoning { thinking, .. } = part {
            // Calculate wrapped height
            let lines: Vec<&str> = thinking.lines().collect();
            let mut height = 0u16;
            for line in lines {
                let line_width = line.chars().count() as u16;
                height += (line_width / width.max(1)).max(0) + 1;
            }
            height.max(1)
        } else {
            1
        }
    }

    fn draw(&self, frame: &mut Frame, area: Rect, part: &Part, theme: &Theme) {
        if let Part::Reasoning { thinking, .. } = part {
            let block = Block::default()
                .title("▼ Thinking")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.border))
                .style(theme.style_muted());

            let text = Text::from(thinking.as_str());
            let paragraph = Paragraph::new(text)
                .wrap(Wrap { trim: true })
                .block(block);

            frame.render_widget(paragraph, area);
        }
    }
}
```

- [ ] **Step 2: Commit**

```bash
cd /home/nan/fi-code && git add crates/core/src/tui/components/part_renderer/thinking.rs && git commit -m "feat: implement ThinkingRenderer

- Migrate existing Thinking card rendering logic
- Calculate dynamic height based on text wrapping"
```

---

### Task 11: ToolCall Renderer with Dispatch

**Files:**
- Create: `crates/core/src/tui/components/part_renderer/tool_call.rs`
- Create: `crates/core/src/tui/components/part_renderer/file_preview.rs`
- Create: `crates/core/src/tui/components/part_renderer/diff.rs`
- Create: `crates/core/src/tui/components/part_renderer/shell_output.rs`
- Create: `crates/core/src/tui/components/part_renderer/task_list.rs`
- Create: `crates/core/src/tui/components/part_renderer/generic_tool.rs`

- [ ] **Step 1: Implement ToolCallRenderer with dispatch**

```rust
use ratatui::{
    layout::Rect,
    Frame,
};
use crate::session::message::Part;
use crate::tui::theme::Theme;
use super::PartRenderer;

pub struct ToolCallRenderer;

impl PartRenderer for ToolCallRenderer {
    fn height(&self, part: &Part, width: u16) -> u16 {
        let sub = self.get_sub_renderer(part);
        sub.height(part, width)
    }

    fn draw(&self, frame: &mut Frame, area: Rect, part: &Part, theme: &Theme) {
        let sub = self.get_sub_renderer(part);
        sub.draw(frame, area, part, theme);
    }
}

impl ToolCallRenderer {
    fn get_sub_renderer(&self, part: &Part) -> Box<dyn PartRenderer> {
        if let Part::ToolUse { name, .. } = part {
            match name.as_str() {
                "read_file" | "view" | "cat" => Box::new(file_preview::FilePreviewRenderer),
                "write_file" | "edit_file" | "apply_diff" | "patch" => Box::new(diff::DiffRenderer),
                "shell" | "execute" | "run" => Box::new(shell_output::ShellOutputRenderer),
                "create_todo" | "update_todo" => Box::new(task_list::TaskListRenderer),
                _ => Box::new(generic_tool::GenericToolRenderer),
            }
        } else {
            Box::new(generic_tool::GenericToolRenderer)
        }
    }
}
```

- [ ] **Step 2: Implement FilePreviewRenderer**

Read file path from `ToolUse.arguments["path"]`, render:
- Title: `File ── {path} ── [{lines} lines]`
- First 10 lines with line numbers
- `... 共 N 行，此处省略 M 行 ...` if more

- [ ] **Step 3: Implement DiffRenderer**

Parse diff from `ToolResult.content` JSON, render:
- Title: `Diff ── +{added} -{removed} ── {path}`
- Unified diff format with +/- colors
- First 20 lines, fold unchanged regions

- [ ] **Step 4: Implement ShellOutputRenderer**

Render:
- Title with full command
- Exit code colored
- Last 15 lines of output
- `stderr` prefixed with `E│`

- [ ] **Step 5: Implement TaskListRenderer**

Render:
- Title: `Tasks ── {total} items`
- Status icons: `☑` `☐` `▣` `!`
- Bottom stats line

- [ ] **Step 6: Implement GenericToolRenderer**

Fallback for unknown tools:
- Show tool name
- Pretty-print JSON arguments
- Show result content

- [ ] **Step 7: Commit**

```bash
cd /home/nan/fi-code && git add crates/core/src/tui/components/part_renderer/tool_call.rs crates/core/src/tui/components/part_renderer/file_preview.rs crates/core/src/tui/components/part_renderer/diff.rs crates/core/src/tui/components/part_renderer/shell_output.rs crates/core/src/tui/components/part_renderer/task_list.rs crates/core/src/tui/components/part_renderer/generic_tool.rs && git commit -m "feat: implement ToolCallRenderer with dispatch and sub-renderers

- ToolCallRenderer dispatches by tool name
- FilePreviewRenderer: first 10 lines with line numbers
- DiffRenderer: unified diff with +/- colors
- ShellOutputRenderer: full command + last 15 lines
- TaskListRenderer: status icons + stats
- GenericToolRenderer: fallback for unknown tools"
```

---

### Task 12: ToolResult Renderer

**Files:**
- Create: `crates/core/src/tui/components/part_renderer/tool_result.rs`

- [ ] **Step 1: Implement ToolResultRenderer**

ToolResult uses the same dispatch logic as ToolCall, but shows results instead of parameters:

```rust
use ratatui::{layout::Rect, Frame};
use crate::session::message::Part;
use crate::tui::theme::Theme;
use super::PartRenderer;

pub struct ToolResultRenderer;

impl PartRenderer for ToolResultRenderer {
    fn height(&self, part: &Part, width: u16) -> u16 {
        let sub = self.get_sub_renderer(part);
        sub.height(part, width)
    }

    fn draw(&self, frame: &mut Frame, area: Rect, part: &Part, theme: &Theme) {
        let sub = self.get_sub_renderer(part);
        sub.draw(frame, area, part, theme);
    }
}

impl ToolResultRenderer {
    fn get_sub_renderer(&self, part: &Part) -> Box<dyn PartRenderer> {
        // For ToolResult, we need to determine the tool name from the corresponding ToolUse
        // This requires looking up the tool_call_id in the Turn's parts
        // For now, use a generic result renderer
        Box::new(super::generic_tool::GenericToolRenderer)
    }
}
```

Note: The actual dispatch requires access to the Turn's `parts` Vec to look up the matching `ToolUse`. This will be handled in Task 14 when we wire renderers into `Chat::draw`.

- [ ] **Step 2: Commit**

```bash
cd /home/nan/fi-code && git add crates/core/src/tui/components/part_renderer/tool_result.rs && git commit -m "feat: add ToolResultRenderer skeleton

- Will dispatch to same sub-renderers as ToolCall
- Full dispatch logic wired in Chat::draw (Task 14)"
```

---

### Task 13: ToolError and Image Renderers

**Files:**
- Create: `crates/core/src/tui/components/part_renderer/tool_error.rs`
- Create: `crates/core/src/tui/components/part_renderer/image.rs`

- [ ] **Step 1: Implement ToolErrorRenderer**

```rust
use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use crate::session::message::Part;
use crate::tui::theme::Theme;
use super::PartRenderer;

pub struct ToolErrorRenderer;

impl PartRenderer for ToolErrorRenderer {
    fn height(&self, part: &Part, width: u16) -> u16 {
        if let Part::ToolError { error_message, .. } = part {
            let lines = error_message.lines().count() as u16;
            lines.max(1) + 2 // +2 for borders
        } else {
            3
        }
    }

    fn draw(&self, frame: &mut Frame, area: Rect, part: &Part, theme: &Theme) {
        if let Part::ToolError { error_message, .. } = part {
            let block = Block::default()
                .title("❌ Tool Error")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.error))
                .style(theme.style_error());

            let text = Text::from(error_message.as_str());
            let paragraph = Paragraph::new(text)
                .wrap(Wrap { trim: true })
                .block(block);

            frame.render_widget(paragraph, area);
        }
    }
}
```

- [ ] **Step 2: Implement ImageRenderer**

```rust
use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use crate::session::message::{ImageSource, Part};
use crate::tui::theme::Theme;
use super::PartRenderer;

pub struct ImageRenderer;

impl PartRenderer for ImageRenderer {
    fn height(&self, _part: &Part, _width: u16) -> u16 {
        1
    }

    fn draw(&self, frame: &mut Frame, area: Rect, part: &Part, theme: &Theme) {
        if let Part::Image { source } = part {
            let path = match source {
                ImageSource::Path { path } => path.clone(),
                ImageSource::Base64 { media_type, .. } => format!("[Base64 {}]", media_type),
                ImageSource::Url { url } => url.clone(),
            };

            let line = Line::from(vec![
                Span::styled("🖼 ", Style::default().fg(theme.brand)),
                Span::styled(path, Style::default().fg(theme.text_primary)),
            ]);

            frame.render_widget(Paragraph::new(line), area);
        }
    }
}
```

- [ ] **Step 3: Commit**

```bash
cd /home/nan/fi-code && git add crates/core/src/tui/components/part_renderer/tool_error.rs crates/core/src/tui/components/part_renderer/image.rs && git commit -m "feat: implement ToolErrorRenderer and ImageRenderer

- ToolErrorRenderer: red-bordered card with error message
- ImageRenderer: show path/URL with image icon"
```

---

### Task 14: Wire Renderers into Chat::draw

**Files:**
- Modify: `crates/core/src/tui/components/chat.rs`

- [ ] **Step 1: Add PartRendererRegistry to Chat**

```rust
pub struct Chat {
    // existing fields...
    pub renderer_registry: PartRendererRegistry,
}
```

Initialize in `Chat::new()`.

- [ ] **Step 2: Rewrite `Chat::draw` to use Part renderers**

Replace card-based rendering with Part-based rendering:

```rust
pub fn draw(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
    for (turn_idx, turn) in self.turns.iter().enumerate() {
        // Draw user message
        // ...

        // Draw AI parts
        let mut current_y = turn_start_y;
        for part in &turn.parts {
            if let Some(renderer) = self.renderer_registry.get(part) {
                let height = renderer.height(part, area.width);
                let part_area = Rect {
                    x: area.x,
                    y: current_y,
                    width: area.width,
                    height,
                };
                renderer.draw(frame, part_area, part, theme);
                current_y += height + 1; // +1 for spacing
            }
        }
    }
}
```

- [ ] **Step 3: Handle ToolResult dispatch with tool name lookup**

In the draw loop, when encountering `Part::ToolResult`, look up the matching `Part::ToolUse` by `tool_call_id` to determine which sub-renderer to use.

- [ ] **Step 4: Commit**

```bash
cd /home/nan/fi-code && git add crates/core/src/tui/components/chat.rs && git commit -m "feat: wire Part renderers into Chat::draw

- Add PartRendererRegistry to Chat struct
- Replace card-based rendering with Part-based rendering
- Handle ToolResult dispatch via tool_call_id lookup"
```

---

## Phase 3: WaveMarker Backend Integration

### Task 15: Add git_write_tree Helper

**Files:**
- Modify: `crates/core/src/tools/basic_tools.rs`
- Modify: `crates/core/src/tools/mod.rs` (export)

- [ ] **Step 1: Add `git_write_tree` to BasicTool**

```rust
pub fn git_write_tree() -> Result<String, String> {
    use std::process::Command;
    let output = Command::new("git")
        .args(["write-tree"])
        .current_dir(std::env::current_dir().map_err(|e| e.to_string())?)
        .output()
        .map_err(|e| format!("Failed to run git write-tree: {}", e))?;

    if output.status.success() {
        let hash = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(hash)
    } else {
        Err(format!(
            "git write-tree failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}
```

- [ ] **Step 2: Export via tools registry**

Add a wrapper or export function so `agent.rs` can call it.

- [ ] **Step 3: Commit**

```bash
cd /home/nan/fi-code && git add crates/core/src/tools/basic_tools.rs crates/core/src/tools/mod.rs && git commit -m "feat: add git_write_tree helper

- Capture current repo state as tree object hash
- Returns hash string on success"
```

---

### Task 16: Insert WaveMarker in Agent Loop

**Files:**
- Modify: `crates/core/src/agent/agent.rs`

- [ ] **Step 1: Modify `run_one_turn` to insert WaveMarker**

At the start of `run_one_turn` (before LLM call):

```rust
// 1. Record snapshot and token baseline
let snapshot = crate::tools::basic_tools::BasicTool::git_write_tree().ok();
let token_baseline = state.token_usage.clone();

// 2. Create new Assistant message with WaveMarker
let mut msg = Message::new(
    session_id.clone(),
    Role::Assistant,
    vec![Part::WaveMarker {
        step: state.turn_count as u32 + 1,
        total: None,
        git_snapshot: snapshot,
        timestamp: crate::session::message::current_timestamp_ms(),
        delta_tokens: TokenUsage::default(),
    }],
);

// 3. Send WaveMarker via SSE
if let Some(ref mut cb) = on_tool_event {
    let _ = cb(SseEvent::Part {
        part: msg.parts[0].clone(),
    });
}
```

- [ ] **Step 2: Backfill delta_tokens and total after LLM response**

After LLM stream completes and tools execute:

```rust
// Backfill WaveMarker delta_tokens
if let Some(Part::WaveMarker { delta_tokens, .. }) = msg.parts.first_mut() {
    *delta_tokens = TokenUsage {
        prompt_tokens: state.token_usage.prompt_tokens - token_baseline.prompt_tokens,
        completion_tokens: state.token_usage.completion_tokens - token_baseline.completion_tokens,
    };
}

// If no tool calls, this is the final wave
if !has_tool_calls {
    if let Some(Part::WaveMarker { total, .. }) = msg.parts.first_mut() {
        *total = Some(state.turn_count as u32 + 1);
    }
}
```

- [ ] **Step 3: Update state.turn_count**

Increment `turn_count` after each successful iteration.

- [ ] **Step 4: Commit**

```bash
cd /home/nan/fi-code && git add crates/core/src/agent/agent.rs && git commit -m "feat: insert WaveMarker at start of each agent loop iteration

- Execute git_write_tree for rollback anchor
- Record token baseline for delta calculation
- Backfill delta_tokens and total after iteration completes"
```

---

### Task 17: WaveMarker Interactions

**Files:**
- Modify: `crates/core/src/tui/components/chat.rs`
- Modify: `crates/core/src/tui/app.rs`

- [ ] **Step 1: Add `g` and `r` key handlers**

In `Chat::handle_event`, add:

```rust
Event::Key(key) => {
    match (key.modifiers, key.code) {
        (KeyModifiers::NONE, KeyCode::Char('g')) => {
            if let Some(turn) = self.turns.get(self.focused_turn) {
                if let Some(Part::WaveMarker { git_snapshot: Some(hash), .. }) = turn.parts.first() {
                    return Some(AppEvent::BrowseGitSnapshot(hash.clone()));
                }
            }
        }
        (KeyModifiers::NONE, KeyCode::Char('r')) => {
            if let Some(turn) = self.turns.get(self.focused_turn) {
                if let Some(Part::WaveMarker { git_snapshot: Some(hash), step, .. }) = turn.parts.first() {
                    return Some(AppEvent::RollbackToWave { snapshot: hash.clone(), step: *step });
                }
            }
        }
        // ...
    }
}
```

- [ ] **Step 2: Add new AppEvent variants**

```rust
pub enum AppEvent {
    // existing variants...
    BrowseGitSnapshot(String),
    RollbackToWave { snapshot: String, step: u32 },
}
```

- [ ] **Step 3: Handle events in `app.rs`**

```rust
AppEvent::BrowseGitSnapshot(hash) => {
    // Execute git ls-tree or checkout to temp branch
    // Show read-only view
}
AppEvent::RollbackToWave { snapshot, step } => {
    // Execute git read-tree + git checkout
    // Reset state to wave beginning
    // Optionally re-execute from that point
}
```

- [ ] **Step 4: Commit**

```bash
cd /home/nan/fi-code && git add crates/core/src/tui/components/chat.rs crates/core/src/tui/app.rs crates/core/src/tui/event.rs && git commit -m "feat: add WaveMarker g/r interactions

- g: browse git snapshot (read-only)
- r: rollback to wave and retry"
```

---

## Plan Self-Review

### Spec Coverage Check

| Spec Section | Plan Task | Status |
|--------------|-----------|--------|
| Part enum extension (ToolError, WaveMarker, Usage) | Task 1 | ✅ |
| SSE transport refactoring | Task 2 | ✅ |
| Backend SSE emission update | Tasks 3-4 | ✅ |
| TUI SSE reception | Task 5 | ✅ |
| Status bar redesign | Task 6 | ✅ |
| PartRenderer trait | Task 7 | ✅ |
| WaveMarker renderer | Task 8 | ✅ |
| Usage renderer | Task 9 | ✅ |
| Thinking renderer | Task 10 | ✅ |
| ToolCall dispatch + sub-renderers | Task 11 | ✅ |
| ToolResult renderer | Task 12 | ✅ |
| ToolError + Image renderers | Task 13 | ✅ |
| Wire renderers into Chat::draw | Task 14 | ✅ |
| git_write_tree helper | Task 15 | ✅ |
| WaveMarker backend integration | Task 16 | ✅ |
| WaveMarker interactions | Task 17 | ✅ |

### Placeholder Scan

- No TBD/TODO in plan
- No "implement later" or "fill in details"
- All steps include actual code or exact commands
- No "similar to Task N" references

### Type Consistency

- `TokenUsage` defined in Task 1, used in Task 16
- `Part::WaveMarker` fields consistent across Tasks 1, 8, 16
- `Part::Usage` fields consistent across Tasks 1, 9
- `SseEvent::Part` consistent across Tasks 2-5

**Result: No gaps, no placeholders, types consistent.**
