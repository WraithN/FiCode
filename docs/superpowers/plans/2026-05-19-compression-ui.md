# Compression UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 fi-code 的上下文压缩功能添加跨平台（TUI / Web / Desktop）的实时 UI 展示能力。

**Architecture:** 后端通过 SSE `CompressionStatus` 事件推送压缩状态，前端状态栏实时显示上下文占比进度条（带颜色编码），压缩完成后在聊天流中插入 `SystemNotice` 通知。

**Tech Stack:** Rust, React/TypeScript, SSE, Zustand

---

## File Structure

| File | Action | Responsibility |
|------|--------|---------------|
| `crates/shared/src/dto.rs` | Modify | 新增 `SseEvent::CompressionStatus` 和 `Part::SystemNotice` |
| `crates/core/src/agent/compression.rs` | Modify | `compress_history` 发送 SSE 事件，计算压缩率 |
| `crates/core/src/agent/agent.rs` | Modify | `run_one_turn` / `agent_loop` 新增 `sse_sender` 参数 |
| `crates/core/src/server/api/chat_api.rs` | Modify | 传递 `sse_sender` 到 `agent_loop` |
| `crates/tui/src/components/status_bar.rs` | Modify | 新增压缩状态，修改 `render_ctx_bar` |
| `crates/tui/src/app.rs` | Modify | 处理 `CompressionStatus` SSE 事件 |
| `frontend/src/types/sse.ts` | Modify | 新增 `compression_status` 类型 |
| `frontend/src/types/part.ts` | Modify | 新增 `system_notice` 类型 |
| `frontend/src/stores/compressionStore.ts` | Create | 压缩状态全局存储 |
| `frontend/src/components/layout/StatusBar.tsx` | Modify | 显示 ctx 占比和压缩状态 |
| `frontend/src/hooks/useChatStream.ts` | Modify | 处理 `compression_status` 事件 |
| `frontend/src/components/part-renderers/SystemNoticePart.tsx` | Create | 系统通知渲染组件 |
| `frontend/src/components/part-renderers/registry.tsx` | Modify | 注册 `system_notice` 渲染器 |

---

## Task 1: 扩展共享 DTO（SSE 事件 + Part 变体）

**Files:**
- Modify: `crates/shared/src/dto.rs`

- [ ] **Step 1: 在 `SseEvent` 枚举中添加 `CompressionStatus`**

在 `SseEvent` 枚举的 `Done` 变体之后添加：

```rust
    /// 压缩状态更新事件
    #[serde(rename = "compression_status")]
    CompressionStatus {
        is_compressing: bool,
        progress: u8,
        context_ratio: u8,
        summary: Option<String>,
    },
```

- [ ] **Step 2: 在 `Part` 枚举中添加 `SystemNotice`**

在 `Part` 枚举的 `Usage` 变体之后添加：

```rust
    /// 系统通知（如压缩完成、Agent 切换等）
    #[serde(rename = "system_notice")]
    SystemNotice {
        kind: String,
        content: String,
    },
```

- [ ] **Step 3: 编译检查**

Run: `cargo check -p fi-code-shared`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/shared/src/dto.rs
git commit -m "feat: add CompressionStatus SseEvent and SystemNotice Part variants"
```

---

## Task 2: 后端压缩模块发送 SSE 事件

**Files:**
- Modify: `crates/core/src/agent/compression.rs`

- [ ] **Step 1: 修改 `compress_history` 函数签名和实现**

修改函数签名，新增 `sse_sender` 参数：

```rust
pub async fn compress_history(
    loop_state: &LoopState,
    client: &dyn AIClient,
    sse_sender: &crate::server::transport::sse::SseSender,
) -> Result<Message> {
```

在函数开头添加：

```rust
    let original_count = loop_state.messages.len();
    let original_tokens = estimate_total_tokens(&loop_state.messages);

    // 发送压缩开始事件
    let _ = sse_sender.send(crate::server::transport::sse::SseEvent::CompressionStatus {
        is_compressing: true,
        progress: 0,
        context_ratio: calculate_context_ratio(loop_state),
        summary: None,
    }).await;
```

在返回之前添加：

```rust
    let token_savings = if original_tokens > 0 {
        let saved = original_tokens.saturating_sub(estimate_tokens(&summary_text));
        ((saved as f64 / original_tokens as f64) * 100.0) as u8
    } else { 0 };

    let display_text = format!(
        "🗜️ 上下文已压缩 | {}条消息 → 1条摘要 | 节省 {}% tokens",
        original_count, token_savings
    );

    // 发送压缩完成事件
    let _ = sse_sender.send(crate::server::transport::sse::SseEvent::CompressionStatus {
        is_compressing: false,
        progress: 100,
        context_ratio: calculate_context_ratio(loop_state),
        summary: Some(display_text.clone()),
    }).await;

    // 在聊天流中插入系统通知
    let _ = sse_sender.send(crate::server::transport::sse::SseEvent::Part {
        part: Part::SystemNotice {
            kind: "compression_done".to_string(),
            content: display_text,
        },
    }).await;
```

- [ ] **Step 2: 添加辅助函数 `calculate_context_ratio`**

在 `compression.rs` 中添加：

```rust
fn calculate_context_ratio(loop_state: &LoopState) -> u8 {
    let limit = get_context_limit();
    let current = estimate_total_tokens(&loop_state.messages);
    if limit == 0 { return 0; }
    ((current as f64 / limit as f64) * 100.0).min(100.0) as u8
}
```

- [ ] **Step 3: 编译检查**

Run: `cargo check -p fi-code-core`
Expected: 可能有错误，因为 `run_one_turn` 还没改签名

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/agent/compression.rs
git commit -m "feat: compress_history sends CompressionStatus SSE events"
```

---

## Task 3: Agent 循环传递 sse_sender

**Files:**
- Modify: `crates/core/src/agent/agent.rs`
- Modify: `crates/core/src/agent/mod.rs`（如有需要）

- [ ] **Step 1: 修改 `run_one_turn` 签名**

将：
```rust
pub async fn run_one_turn<C: AIClient + ?Sized>(
    client: &C,
    state: &mut LoopState,
    agent_type: AgentType,
    on_text: &mut Option<Box<dyn FnMut(&str) + Send>>,
    on_tool_event: &mut Option<Box<dyn FnMut(crate::server::transport::sse::SseEvent) + Send>>,
) -> Result<bool> {
```

改为：
```rust
pub async fn run_one_turn<C: AIClient + ?Sized>(
    client: &C,
    state: &mut LoopState,
    agent_type: AgentType,
    on_text: &mut Option<Box<dyn FnMut(&str) + Send>>,
    on_tool_event: &mut Option<Box<dyn FnMut(crate::server::transport::sse::SseEvent) + Send>>,
    sse_sender: &crate::server::transport::sse::SseSender,
) -> Result<bool> {
```

- [ ] **Step 2: 修改 `agent_loop` 签名**

将：
```rust
pub async fn agent_loop<C: AIClient + ?Sized>(
    client: &C,
    state: &mut LoopState,
    agent_type: AgentType,
    on_text: &mut Option<Box<dyn FnMut(&str) + Send>>,
    on_tool_event: &mut Option<Box<dyn FnMut(crate::server::transport::sse::SseEvent) + Send>>,
) -> Result<()> {
```

改为：
```rust
pub async fn agent_loop<C: AIClient + ?Sized>(
    client: &C,
    state: &mut LoopState,
    agent_type: AgentType,
    on_text: &mut Option<Box<dyn FnMut(&str) + Send>>,
    on_tool_event: &mut Option<Box<dyn FnMut(crate::server::transport::sse::SseEvent) + Send>>,
    sse_sender: &crate::server::transport::sse::SseSender,
) -> Result<()> {
```

- [ ] **Step 3: 在 `agent_loop` 中传递 `sse_sender` 给 `run_one_turn`**

找到 `run_one_turn` 调用，添加 `sse_sender` 参数。

- [ ] **Step 4: 在 `run_one_turn` 中传递 `sse_sender` 给 `compress_history`**

找到 `compress_history` 调用，添加 `sse_sender` 参数。

- [ ] **Step 5: 编译检查并修复所有调用方**

Run: `cargo check -p fi-code-core`
Expected: 会有错误，因为 `chat_api.rs` 和测试中的 `agent_loop` / `run_one_turn` 调用还没改

搜索所有调用方：
```bash
grep -rn "agent_loop(" crates/ --include="*.rs"
grep -rn "run_one_turn(" crates/ --include="*.rs"
```

修复所有调用方，添加 `sse_sender` 参数。

- [ ] **Step 6: 重新编译**

Run: `cargo check -p fi-code-core`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add crates/core/src/agent/agent.rs
git commit -m "feat: pass sse_sender through agent_loop and run_one_turn"
```

---

## Task 4: chat_api.rs 传递 sse_sender

**Files:**
- Modify: `crates/core/src/server/api/chat_api.rs`

- [ ] **Step 1: 在 `run_agent_chat` 中传递 `sse_sender` 给 `agent_loop`**

找到 `agent_loop` 调用，添加 `&sse_sender` 参数。

- [ ] **Step 2: 编译检查**

Run: `cargo check -p fi-code-core`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/server/api/chat_api.rs
git commit -m "feat: pass sse_sender from chat_api to agent_loop"
```

---

## Task 5: TUI 状态栏改造

**Files:**
- Modify: `crates/tui/src/components/status_bar.rs`

- [ ] **Step 1: 在 StatusBar 结构中添加压缩状态字段**

```rust
pub struct StatusBar {
    // ... 现有字段 ...
    is_compressing: bool,
    compression_progress: u8,
}
```

在 `new()` 中初始化：
```rust
is_compressing: false,
compression_progress: 0,
```

- [ ] **Step 2: 添加 setter 方法**

```rust
pub fn set_compressing(&mut self, compressing: bool) {
    self.is_compressing = compressing;
}

pub fn set_compression_progress(&mut self, progress: u8) {
    self.compression_progress = progress;
}
```

- [ ] **Step 3: 修改 `render_ctx_bar()`**

将现有实现替换为：

```rust
fn render_ctx_bar(&self) -> String {
    let ratio = if self.ctx_limit == 0 {
        0.0
    } else {
        (self.ctx_current as f64 / self.ctx_limit as f64).min(1.0)
    };
    let filled = ((ratio * CTX_BAR_WIDTH as f64).ceil() as usize).min(CTX_BAR_WIDTH);
    let empty = CTX_BAR_WIDTH - filled;
    let pct = (ratio * 100.0) as u8;

    if self.is_compressing {
        let c_filled = ((self.compression_progress as f64 / 100.0) * CTX_BAR_WIDTH as f64)
            .ceil() as usize;
        let c_empty = CTX_BAR_WIDTH - c_filled;
        format!("[{}{}] 🗜️", "█".repeat(c_filled), "░".repeat(c_empty))
    } else {
        format!("[{}{}] {}%", "█".repeat(filled), "░".repeat(empty), pct)
    }
}
```

- [ ] **Step 4: 微调颜色阈值**

将 `ctx_bar_style` 中的阈值从 `0.8` / `0.5` 改为 `0.85` / `0.6`：

```rust
let color = if ratio > 0.85 {
    theme.error
} else if ratio > 0.60 {
    theme.warning
} else {
    theme.success
};
```

- [ ] **Step 5: 编译检查**

Run: `cargo check -p fi-code-tui`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/tui/src/components/status_bar.rs
git commit -m "feat: TUI status bar shows context ratio and compression progress"
```

---

## Task 6: TUI 事件处理

**Files:**
- Modify: `crates/tui/src/app.rs`

- [ ] **Step 1: 在 `handle_app_event` 中添加 `CompressionStatus` 处理**

在 `AppEvent::SseEvent` 的 match 分支中，在 `SseEvent::Part { part: Part::Usage { ... } }` 处理之后，添加：

```rust
SseEvent::CompressionStatus {
    is_compressing,
    progress,
    context_ratio,
    summary,
} => {
    self.status_bar.set_compressing(*is_compressing);
    self.status_bar.set_compression_progress(*progress);
    if *context_ratio > 0 {
        let current = (self.status_bar.ctx_limit() as f64 * (*context_ratio as f64 / 100.0)) as usize;
        self.status_bar.set_ctx_tokens(current, self.status_bar.ctx_limit());
    }
    if !is_compressing && summary.is_some() {
        self.chat.add_system_message(summary.as_ref().unwrap());
    }
}
```

注意：`self.status_bar.ctx_limit()` 需要添加 getter 方法，或者直接用 `DEFAULT_CONTEXT_LIMIT`。

- [ ] **Step 2: 添加 `ctx_limit` getter（如需要）**

如果 `StatusBar` 没有 `ctx_limit()` getter，在 `status_bar.rs` 中添加：

```rust
pub fn ctx_limit(&self) -> usize {
    self.ctx_limit
}
```

- [ ] **Step 3: 编译检查**

Run: `cargo check -p fi-code-tui`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/tui/src/app.rs crates/tui/src/components/status_bar.rs
git commit -m "feat: TUI handles CompressionStatus SSE events"
```

---

## Task 7: 前端类型扩展

**Files:**
- Modify: `frontend/src/types/sse.ts`
- Modify: `frontend/src/types/part.ts`

- [ ] **Step 1: 扩展 `SseEvent` 类型**

```typescript
export type SseEvent =
  | { type: 'message'; content: string }
  | { type: 'part'; part: Part }
  | { type: 'agent_info'; agent_type: 'build' | 'plan'; agent_name: string }
  | { type: 'done'; session_id: string }
  | { type: 'error'; message: string }
  | { type: 'task_progress'; plan_id: string; tasks: TaskProgressItem[] }
  | { type: 'compression_status'; is_compressing: boolean; progress: number; context_ratio: number; summary?: string };
```

- [ ] **Step 2: 扩展 `Part` 类型**

```typescript
export type Part =
  | { type: 'text'; text: string }
  | { type: 'image'; source: ImageSource }
  | { type: 'tool_use'; id: string; name: string; arguments: Record<string, unknown> }
  | { type: 'tool_result'; tool_call_id: string; content: string; duration_ms?: number }
  | { type: 'tool_error'; tool_call_id: string; content: string; error_message: string }
  | { type: 'reasoning'; thinking: string; signature?: string }
  | { type: 'wave_marker'; step: number; total?: number; git_snapshot?: string; timestamp: number; delta_tokens: TokenUsage }
  | { type: 'usage'; prompt_tokens: number; completion_tokens: number; latency_ms: number; cost?: number }
  | { type: 'system_notice'; kind: string; content: string };
```

- [ ] **Step 3: 编译检查（前端）**

Run: `cd frontend && npx tsc --noEmit`
Expected: 可能有错误，因为其他文件还没修改

- [ ] **Step 4: Commit**

```bash
git add frontend/src/types/sse.ts frontend/src/types/part.ts
git commit -m "feat: add compression_status and system_notice types"
```

---

## Task 8: 前端压缩状态存储

**Files:**
- Create: `frontend/src/stores/compressionStore.ts`

- [ ] **Step 1: 创建 compressionStore**

```typescript
import { create } from 'zustand';

interface CompressionState {
  isCompressing: boolean;
  progress: number;
  contextRatio: number;
  setCompressionStatus: (status: { isCompressing: boolean; progress: number; contextRatio: number }) => void;
}

export const useCompressionStore = create<CompressionState>((set) => ({
  isCompressing: false,
  progress: 0,
  contextRatio: 0,
  setCompressionStatus: (status) => set(status),
}));
```

- [ ] **Step 2: Commit**

```bash
git add frontend/src/stores/compressionStore.ts
git commit -m "feat: add compressionStore for frontend"
```

---

## Task 9: 前端 StatusBar 改造

**Files:**
- Modify: `frontend/src/components/layout/StatusBar.tsx`

- [ ] **Step 1: 导入 compressionStore**

```typescript
import { useCompressionStore } from '../../stores/compressionStore';
```

- [ ] **Step 2: 在组件中使用压缩状态**

```typescript
const { isCompressing, progress, contextRatio } = useCompressionStore();
```

- [ ] **Step 3: 添加上下文占比显示**

在 AGT 显示之后添加：

```typescript
const ratioColor = contextRatio > 85 ? 'text-error' : contextRatio > 60 ? 'text-warning' : 'text-success';
const filled = Math.ceil((contextRatio / 100) * 10);
const ctxBar = '█'.repeat(filled) + '░'.repeat(10 - filled);

// ...

<span className="mx-2 text-border">│</span>
<span className={`${ratioColor} font-mono`}>
  CTX: [{ctxBar}] {contextRatio}% 12.3k/128k
</span>
```

注意：这里的 `12.3k/128k` 是占位符，实际应该从后端获取当前/上限 token 数。如果前端目前没有这些值，可以暂时只显示百分比。

- [ ] **Step 4: 添加压缩状态显示**

在 `isGenerating` 显示之前添加：

```typescript
{isCompressing && (
  <>
    <span className="mx-2 text-border">│</span>
    <span className="text-brand animate-pulse">🗜️ Compressing {progress}%...</span>
  </>
)}
```

并修改 `isGenerating` 的条件为 `isGenerating && !isCompressing`。

- [ ] **Step 5: 编译检查**

Run: `cd frontend && npx tsc --noEmit`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add frontend/src/components/layout/StatusBar.tsx
git commit -m "feat: Web/Desktop status bar shows context ratio and compression progress"
```

---

## Task 10: 前端 SSE 处理

**Files:**
- Modify: `frontend/src/hooks/useChatStream.ts`

- [ ] **Step 1: 导入 compressionStore**

```typescript
import { useCompressionStore } from '../stores/compressionStore';
```

- [ ] **Step 2: 在 `useChatStream` 中获取 `setCompressionStatus`**

```typescript
const { setCompressionStatus } = useCompressionStore();
```

- [ ] **Step 3: 修改 `handleSseEvent` 函数签名**

添加 `setCompressionStatus` 参数：

```typescript
function handleSseEvent(
  event: SseEvent,
  turnId: string,
  setAgent: (agent: 'build' | 'plan') => void,
  appendPart: (turnId: string, part: Part) => void,
  completeTurn: (turnId: string) => void,
  setCurrentSessionId: (id: string | null) => void,
  setIsGenerating: (generating: boolean) => void,
  setCompressionStatus: (status: { isCompressing: boolean; progress: number; contextRatio: number }) => void,
)
```

- [ ] **Step 4: 添加 `compression_status` case**

```typescript
case 'compression_status':
  setCompressionStatus({
    isCompressing: event.is_compressing,
    progress: event.progress,
    contextRatio: event.context_ratio,
  });
  if (!event.is_compressing && event.summary) {
    appendPart(turnId, { type: 'system_notice', kind: 'compression_done', content: event.summary });
  }
  break;
```

- [ ] **Step 5: 更新 `handleSseEvent` 调用**

在 `send` 函数中，将 `handleSseEvent` 调用添加 `setCompressionStatus` 参数。

- [ ] **Step 6: 编译检查**

Run: `cd frontend && npx tsc --noEmit`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add frontend/src/hooks/useChatStream.ts
git commit -m "feat: frontend handles compression_status SSE events"
```

---

## Task 11: SystemNotice Part 渲染器

**Files:**
- Create: `frontend/src/components/part-renderers/SystemNoticePart.tsx`
- Modify: `frontend/src/components/part-renderers/registry.tsx`

- [ ] **Step 1: 创建 SystemNoticePart 组件**

```typescript
import React from 'react';

export const SystemNoticePart: React.FC<{ part: { type: 'system_notice'; kind: string; content: string } }> = ({ part }) => (
  <div className="my-2 px-3 py-2 bg-bg-overlay border-l-2 border-brand rounded text-sm text-text-muted italic">
    {part.content}
  </div>
);
```

- [ ] **Step 2: 注册到 registry**

在 `registry.tsx` 中导入并注册：

```typescript
import { SystemNoticePart } from './SystemNoticePart';

export const partRenderers = {
  // ... 现有渲染器 ...
  system_notice: SystemNoticePart as React.FC<{ part: Part }>,
};
```

- [ ] **Step 3: 编译检查**

Run: `cd frontend && npx tsc --noEmit`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add frontend/src/components/part-renderers/SystemNoticePart.tsx frontend/src/components/part-renderers/registry.tsx
git commit -m "feat: add SystemNoticePart renderer"
```

---

## Task 12: 完整测试验证

- [ ] **Step 1: 运行后端测试**

Run: `cargo test -p fi-code-core`
Expected: 除预先存在的 `test_tool_call_web_fetch_success` 外全部通过

- [ ] **Step 2: 运行 TUI 测试**

Run: `cargo test -p fi-code-tui`
Expected: 全部通过

- [ ] **Step 3: 运行前端构建**

Run: `cd frontend && npm run build`
Expected: PASS

- [ ] **Step 4: 运行 Clippy**

Run: `cargo clippy -p fi-code-core -p fi-code-tui -- -D warnings`
Expected: PASS（或只有预先存在的 warning）

- [ ] **Step 5: Commit**

```bash
git commit -m "feat: compression UI complete - all tests pass" --allow-empty
```

---

## Spec Coverage Checklist

| Spec 要求 | 对应 Task |
|-----------|-----------|
| 状态栏上下文占比（进度条 + 百分比 + 颜色） | Task 5 (TUI), Task 9 (Web) |
| 压缩过程中展示"🗜️ Compressing..." | Task 2 (后端), Task 5 (TUI), Task 9 (Web) |
| 压缩完毕后展示压缩率 | Task 2 (后端), Task 6 (TUI), Task 10 (Web) |
| 底层状态栏新增当前上下文占用率 | Task 5 (TUI), Task 9 (Web) |
| 跨平台一致（TUI/Web/Desktop） | Task 5-6 (TUI), Task 7-11 (Web) |
| 压缩通知插入聊天流 | Task 2 (后端), Task 6 (TUI), Task 10 (Web) |

---

## Execution Options

**Plan complete and saved to `docs/superpowers/plans/2026-05-19-compression-ui.md`.**

**1. Subagent-Driven (recommended)** - Fresh subagent per task, review between tasks

**2. Inline Execution** - Execute tasks in this session

**Which approach?**