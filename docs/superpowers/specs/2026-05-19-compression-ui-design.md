# Context Compression UI Design

> 为 fi-code 的上下文压缩功能添加跨平台（TUI / Web / Desktop）的实时展示能力，包括状态栏上下文占比、压缩进度通知和压缩结果反馈。

---

## 1. 背景与动机

上下文压缩模块已在后端实现，但用户无法感知压缩何时发生、压缩了多少、当前上下文还剩多少空间。本设计为压缩功能补齐 UI 展示层，让用户对会话状态有直观了解。

---

## 2. 设计目标

1. **状态栏上下文占比**：TUI/Web/Desktop 状态栏实时显示当前上下文占用率（进度条 + 百分比 + 颜色编码）
2. **压缩进度通知**：压缩过程中展示"🗜️ Compressing..."，压缩完成后展示压缩结果
3. **压缩率展示**：显示压缩了多少条消息、节省了多少 % tokens
4. **跨平台一致**：TUI 和 Web/Desktop 展示信息完全一致

---

## 3. 架构设计

### 3.1 数据流

```
[Backend]
  compress_history()
    ├── SseEvent::CompressionStatus { is_compressing: true, progress: 0 }
    ├── ... compressing ...
    ├── SseEvent::CompressionStatus { is_compressing: false, progress: 100, summary }
    └── SseEvent::Part { Part::SystemNotice { kind: "compression_done", content } }

[TUI]
  app.rs::handle_app_event()
    ├── SseEvent::CompressionStatus → status_bar.set_compressing() / set_compression_progress()
    └── SseEvent::Part::SystemNotice → chat.add_system_message()

[Web/Desktop]
  useChatStream::handleSseEvent()
    ├── compression_status → compressionStore.setCompressionStatus()
    └── part::system_notice → appendPart(turnId, part)
```

---

## 4. 详细设计

### 4.1 共享 DTO 扩展

**`crates/shared/src/dto.rs`**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SseEvent {
    // ... 现有变体 ...

    /// 压缩状态更新事件
    #[serde(rename = "compression_status")]
    CompressionStatus {
        is_compressing: bool,
        progress: u8,
        context_ratio: u8,
        summary: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Part {
    // ... 现有变体 ...

    /// 系统通知（如压缩完成、Agent 切换等）
    #[serde(rename = "system_notice")]
    SystemNotice {
        kind: String,
        content: String,
    },
}
```

### 4.2 后端压缩事件发送

**`crates/core/src/agent/compression.rs`**

```rust
pub async fn compress_history(
    loop_state: &LoopState,
    client: &dyn AIClient,
    sse_sender: &SseSender,
) -> Result<Message> {
    let original_count = loop_state.messages.len();
    let original_tokens = estimate_total_tokens(&loop_state.messages);

    // 发送压缩开始
    let _ = sse_sender.send(SseEvent::CompressionStatus {
        is_compressing: true,
        progress: 0,
        context_ratio: calculate_context_ratio(loop_state),
        summary: None,
    }).await;

    let range = find_compression_range(&loop_state.messages)
        .ok_or_else(|| anyhow::anyhow!("No compressible range"))?;

    // ... 构建待压缩消息，调用 subagent ...

    let summary_text = run_compression_subagent(client, to_compress).await?;
    let token_savings = calculate_token_savings(original_tokens, &summary_text);

    let display_text = format!(
        "🗜️ 上下文已压缩 | {}条消息 → 1条摘要 | 节省 {}% tokens",
        original_count, token_savings
    );

    // 发送压缩完成
    let _ = sse_sender.send(SseEvent::CompressionStatus {
        is_compressing: false,
        progress: 100,
        context_ratio: calculate_context_ratio(loop_state),
        summary: Some(display_text.clone()),
    }).await;

    // 在聊天流中插入系统通知
    let _ = sse_sender.send(SseEvent::Part {
        part: Part::SystemNotice {
            kind: "compression_done".to_string(),
            content: display_text,
        },
    }).await;

    Ok(Message::new(
        session_id,
        Role::User,
        vec![Part::Text { text: summary_text }],
    ))
}
```

### 4.3 Agent 循环传递 sse_sender

**`crates/core/src/agent/agent.rs`**

修改 `run_one_turn` 和 `agent_loop` 签名，新增 `sse_sender: &SseSender` 参数。

### 4.4 TUI 状态栏

**`crates/tui/src/components/status_bar.rs`**

#### 新增字段
```rust
pub struct StatusBar {
    // ... 现有字段 ...
    is_compressing: bool,
    compression_progress: u8,
}
```

#### 修改 `render_ctx_bar()`
始终显示上下文占比，移除 Running 状态的动画填充：

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

#### 颜色编码
```rust
fn ctx_bar_style(&self, theme: &Theme) -> Style {
    let ratio = if self.ctx_limit > 0 {
        self.ctx_current as f64 / self.ctx_limit as f64
    } else { 0.0 };
    let color = if ratio > 0.85 {
        theme.error
    } else if ratio > 0.60 {
        theme.warning
    } else {
        theme.success
    };
    Style::default().fg(color)
}
```

#### 状态栏布局（标准模式）
```
FiCode │ AGT: Build │ CTX: [████░░░░░░] 45% 12.3k/128k │ TOK: ↑45k ↓12k │ MDL: kimi-k2.5 │ 14:32
```

压缩时：
```
FiCode │ AGT: Build │ CTX: [███░░░░░░░] 🗜️ Compressing... │ MDL: kimi-k2.5 │ 14:32
```

### 4.5 TUI 事件处理

**`crates/tui/src/app.rs`**

```rust
AppEvent::SseEvent(SseEvent::CompressionStatus {
    is_compressing,
    progress,
    context_ratio,
    summary,
}) => {
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

### 4.6 Web/Desktop 前端

#### 新增类型

**`frontend/src/types/sse.ts`**
```typescript
export type SseEvent =
  | // ... 现有类型 ...
  | { type: 'compression_status'; is_compressing: boolean; progress: number; context_ratio: number; summary?: string };
```

**`frontend/src/types/part.ts`**
```typescript
export type Part =
  | // ... 现有类型 ...
  | { type: 'system_notice'; kind: string; content: string };
```

#### 新增状态存储

**`frontend/src/stores/compressionStore.ts`**
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

#### StatusBar 组件

**`frontend/src/components/layout/StatusBar.tsx`**
```tsx
export const StatusBar: React.FC = () => {
  const { currentAgent, setAgent, isGenerating } = useChatStore();
  const { currentModel } = useUIStore();
  const { connectionStatus } = useConnectionStore();
  const { isCompressing, progress, contextRatio } = useCompressionStore();

  const ratioColor = contextRatio > 85 ? 'text-error' : contextRatio > 60 ? 'text-warning' : 'text-success';
  const filled = Math.ceil((contextRatio / 100) * 10);
  const ctxBar = '█'.repeat(filled) + '░'.repeat(10 - filled);

  return (
    <div className="h-8 flex items-center px-4 bg-bg-secondary border-t border-border text-xs select-none">
      <span className="font-bold text-brand">fi-code</span>
      <span className="mx-2 text-border">│</span>
      <button onClick={() => setAgent(currentAgent === 'build' ? 'plan' : 'build')}>
        AGT: {currentAgent === 'build' ? 'Build' : 'Plan'}
      </button>
      <span className="mx-2 text-border">│</span>
      <span className={`${ratioColor} font-mono`}>
        CTX: [{ctxBar}] {contextRatio}% 12.3k/128k
      </span>
      <span className="mx-2 text-border">│</span>
      <span className="text-text-secondary">{currentModel}</span>
      <span className="mx-2 text-border">│</span>
      <span className={connectionStatus === 'connected' ? 'text-success' : 'text-error'}>
        {connectionStatus}
      </span>
      {isCompressing && (
        <><span className="mx-2 text-border">│</span><span className="text-brand animate-pulse">🗜️ Compressing {progress}%...</span></>
      )}
      {isGenerating && !isCompressing && (
        <><span className="mx-2 text-border">│</span><span className="text-brand animate-pulse">generating...</span></>
      )}
    </div>
  );
};
```

#### SSE 处理

**`frontend/src/hooks/useChatStream.ts`**
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

#### SystemNotice Part 渲染器

**`frontend/src/components/part-renderers/SystemNoticePart.tsx`**
```tsx
export const SystemNoticePart: React.FC<{ part: Extract<Part, { type: 'system_notice' }> }> = ({ part }) => (
  <div className="my-2 px-3 py-2 bg-bg-overlay border-l-2 border-brand rounded text-sm text-text-muted italic">
    {part.content}
  </div>
);
```

注册到 `registry.tsx`：
```tsx
system_notice: SystemNoticePart as React.FC<{ part: Part }>,
```

---

## 5. 文件变更清单

| 文件 | 动作 | 说明 |
|------|------|------|
| `crates/shared/src/dto.rs` | 修改 | 新增 `SseEvent::CompressionStatus` 和 `Part::SystemNotice` |
| `crates/core/src/agent/compression.rs` | 修改 | `compress_history` 发送 SSE 事件，计算压缩率 |
| `crates/core/src/agent/agent.rs` | 修改 | `run_one_turn` / `agent_loop` 新增 `sse_sender` 参数 |
| `crates/core/src/server/api/chat_api.rs` | 修改 | 传递 `sse_sender` 到 `agent_loop` |
| `crates/tui/src/components/status_bar.rs` | 修改 | 新增 `is_compressing` / `compression_progress`，修改 `render_ctx_bar` |
| `crates/tui/src/app.rs` | 修改 | 处理 `CompressionStatus` SSE 事件 |
| `frontend/src/types/sse.ts` | 修改 | 新增 `compression_status` 类型 |
| `frontend/src/types/part.ts` | 修改 | 新增 `system_notice` 类型 |
| `frontend/src/stores/compressionStore.ts` | 创建 | 压缩状态全局存储 |
| `frontend/src/components/layout/StatusBar.tsx` | 修改 | 显示 ctx 占比和压缩状态 |
| `frontend/src/hooks/useChatStream.ts` | 修改 | 处理 `compression_status` 事件 |
| `frontend/src/components/part-renderers/SystemNoticePart.tsx` | 创建 | 系统通知渲染组件 |
| `frontend/src/components/part-renderers/registry.tsx` | 修改 | 注册 `system_notice` 渲染器 |

---

## 6. 测试策略

### 6.1 后端测试
- `test_compression_sends_sse_events`：验证压缩开始/结束时发送正确的 SSE 事件
- `test_system_notice_part_serde`：验证 SystemNotice Part 的序列化/反序列化

### 6.2 TUI 测试
- `test_status_bar_ctx_bar_idle`：验证 Idle 状态显示正确的 ctx 占比
- `test_status_bar_ctx_bar_compressing`：验证压缩状态显示压缩进度
- `test_status_bar_ctx_bar_style_thresholds`：验证颜色阈值（>85% 红，>60% 黄）

### 6.3 前端测试
- `test_compression_store_updates`：验证 compressionStore 状态更新
- `test_system_notice_renderer`：验证 SystemNoticePart 渲染

---

## 7. 风险与缓解

| 风险 | 缓解 |
|------|------|
| `sse_sender` 参数传递链路长 | 逐步传递，每层增加一个参数，避免重构 agent_loop 签名时遗漏 |
| TUI 状态栏空间不足（小终端） | 紧凑模式下只显示百分比，不显示进度条 |
| SystemNotice 被模型误解 | SystemNotice 不发送给 LLM，仅用于前端展示 |
| 前端类型与后端不一致 | 修改 `shared/src/dto.rs` 后同步更新前端 TypeScript 类型 |
