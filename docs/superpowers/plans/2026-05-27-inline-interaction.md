# 信息流内联交互 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 PermissionAsk 和 QuestionAsk 从弹窗改为聊天信息流中内联展示，并将所有 Low 风险工具改为自动放行。

**Architecture:** 后端保留 SSE 事件机制不变，仅调整风险等级判断；前端移除 PermissionDialog 弹窗，新增两种交互式 part 类型并在消息流中渲染。

**Tech Stack:** Rust (Tokio, Axum), React (TypeScript, Zustand, TailwindCSS)

---

## 文件结构

| 文件 | 动作 | 职责 |
|------|------|------|
| `crates/core/src/permission/permission.rs` | 修改 | 调整工具风险等级：write/edit 标记为 High，其余 Low 风险工具自动 Allow |
| `frontend/src/types/part.ts` | 修改 | 新增 `interactive_permission` 和 `interactive_question` part 类型 |
| `frontend/src/stores/chatStore.ts` | 修改 | 新增 `updatePart` 方法，支持更新 turn 中特定 part 的状态 |
| `frontend/src/hooks/useChatStream.ts` | 修改 | `permission_ask` / `question_ask` SSE 事件改为 `appendPart` 而非存入 permissionStore |
| `frontend/src/components/layout/AppLayout.tsx` | 修改 | 移除 `<PermissionDialog />` 引用 |
| `frontend/src/components/PermissionDialog.tsx` | 删除 | 弹窗组件不再使用 |
| `frontend/src/components/part-renderers/InteractivePermissionPart.tsx` | 新增 | 在消息流中渲染权限确认按钮组 |
| `frontend/src/components/part-renderers/InteractiveQuestionPart.tsx` | 新增 | 在消息流中渲染问题选项按钮组 |
| `frontend/src/components/part-renderers/index.ts` | 修改 | 导出新增的交互组件 |
| `frontend/src/components/chat/TurnGroup.tsx` 或 `MessageBubble.tsx` | 修改 | part 渲染分发逻辑新增 interactive 分支 |
| `frontend/src/components/part-renderers/ToolUsePart.tsx` | 修改 | 确认 ask_for_question 的过滤逻辑仍然生效 |

---

### Task 1: 后端风险等级调整

**Files:**
- Modify: `crates/core/src/permission/permission.rs:108-126`

- [ ] **Step 1: 修改 match_action 逻辑**

将 `write` 和 `edit` 显式标记为 High 风险，其余工具默认改为 Allow + Low：

```rust
// 在现有 "read_file" / "read" / "grep" 分支之后，"ask_for_question" 分支之前插入：
} else if tool_name == "write" || tool_name == "edit" {
    (
        Self::Ask,
        RiskType::High,
        format!("tool {} may modify files, requires confirmation", tool_name),
    )
} else if tool_name == "ask_for_question" {
    // ... 保持不变
} else {
    (
        Self::Allow,
        RiskType::Low,
        format!("tool {} auto-approved (low risk)", tool_name),
    )
}
```

- [ ] **Step 2: 编译检查**

Run: `cargo check -p fi-code-core`
Expected: 无编译错误

- [ ] **Step 3: 运行 permission 相关单元测试**

Run: `cargo test -p fi-code-core permission`
Expected: 全部通过（如有失败需同步更新测试断言）

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/permission/permission.rs
git commit -m "feat: low-risk tools auto-allow, write/edit marked as high risk"
```

---

### Task 2: 前端新增 Part 类型

**Files:**
- Modify: `frontend/src/types/part.ts`

- [ ] **Step 1: 在 Part 联合类型末尾新增两种交互式 part**

```typescript
| { 
    type: 'interactive_permission'; 
    tool_call_id: string; 
    tool_name: string; 
    risk: string; 
    reason: string; 
    status: 'pending' | 'approved' | 'rejected';
  }
| { 
    type: 'interactive_question'; 
    tool_call_id: string; 
    question: string; 
    options: { id: string; label: string; description?: string }[]; 
    recommended?: string;
    allow_custom: boolean;
    status: 'pending' | 'answered';
    answer?: string;
  }
```

- [ ] **Step 2: Commit**

```bash
git add frontend/src/types/part.ts
git commit -m "feat: add interactive_permission and interactive_question part types"
```

---

### Task 3: chatStore 新增 updatePart 方法

**Files:**
- Modify: `frontend/src/stores/chatStore.ts`

- [ ] **Step 1: 在 store 定义中新增 updatePart**

在现有方法（如 `appendPart`, `completeTurn`）附近添加：

```typescript
updatePart: (turnId: string, partIndex: number, updater: (part: Part) => Part) => {
  set((state) => ({
    turns: state.turns.map((turn) => {
      if (turn.id !== turnId) return turn;
      const newParts = [...turn.parts];
      if (partIndex >= 0 && partIndex < newParts.length) {
        newParts[partIndex] = updater(newParts[partIndex]);
      }
      return { ...turn, parts: newParts };
    }),
  }));
}
```

- [ ] **Step 2: Commit**

```bash
git add frontend/src/stores/chatStore.ts
git commit -m "feat: add updatePart to chatStore for interactive component state updates"
```

---

### Task 4: SSE 事件处理改为 appendPart

**Files:**
- Modify: `frontend/src/hooks/useChatStream.ts`

- [ ] **Step 1: 修改 useChatStream 解构，新增 updatePart**

```typescript
const { startTurn, appendPart, completeTurn, setAgent, setIsGenerating, updatePart } = useChatStore();
```

- [ ] **Step 2: 修改 handleSseEvent 中 permission_ask 和 question_ask 的处理**

将这两个 case 从 `setPendingPermission` / `setPendingQuestion` 改为 `appendPart`：

```typescript
case 'permission_ask':
  appendPart(turnId, {
    type: 'interactive_permission',
    tool_call_id: event.tool_call_id,
    tool_name: event.tool_name,
    risk: event.risk,
    reason: event.reason,
    status: 'pending',
  });
  break;

case 'question_ask':
  appendPart(turnId, {
    type: 'interactive_question',
    tool_call_id: event.tool_call_id,
    question: event.question,
    options: event.options,
    recommended: event.recommended,
    allow_custom: event.allow_custom,
    status: 'pending',
  });
  break;
```

- [ ] **Step 3: 从 useChatStream 中移除 permissionStore 相关引用**

```typescript
// 移除
import { usePermissionStore } from '../stores/permissionStore';
const { setPendingPermission, setPendingQuestion } = usePermissionStore();
```

同时从 `send` 的依赖数组和 `handleSseEvent` 参数中移除这些 setter。

- [ ] **Step 4: Commit**

```bash
git add frontend/src/hooks/useChatStream.ts
git commit -m "feat: convert permission_ask/question_ask SSE to inline parts"
```

---

### Task 5: 新增 InteractivePermissionPart 组件

**Files:**
- Create: `frontend/src/components/part-renderers/InteractivePermissionPart.tsx`

- [ ] **Step 1: 创建组件文件**

```typescript
import React, { useState } from 'react';
import { apiClient } from '../../services/apiClient';
import { useChatStore } from '../../stores/chatStore';

interface Props {
  turnId: string;
  partIndex: number;
  part: Extract<Part, { type: 'interactive_permission' }>;
}

export const InteractivePermissionPart: React.FC<Props> = ({ turnId, partIndex, part }) => {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const updatePart = useChatStore((s) => s.updatePart);

  const handleApprove = async () => {
    if (loading || part.status !== 'pending') return;
    setLoading(true);
    setError(null);
    try {
      await apiClient.respondPermission(part.tool_call_id, true);
      updatePart(turnId, partIndex, (p) => ({ ...(p as any), status: 'approved' }));
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to approve');
    } finally {
      setLoading(false);
    }
  };

  const handleReject = async () => {
    if (loading || part.status !== 'pending') return;
    setLoading(true);
    setError(null);
    try {
      await apiClient.respondPermission(part.tool_call_id, false);
      updatePart(turnId, partIndex, (p) => ({ ...(p as any), status: 'rejected' }));
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to reject');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="my-2 p-4 glass border border-tauri-border rounded-2xl space-y-3">
      <div className="flex items-center gap-2 text-sm text-text-muted">
        <span>工具:</span>
        <span className="font-mono">{part.tool_name}</span>
      </div>
      <div className="flex items-center gap-2 text-sm">
        <span className="text-text-muted">风险等级:</span>
        <span className={`font-semibold ${
          part.risk === 'Critical' ? 'text-red-500' :
          part.risk === 'High' ? 'text-orange-500' : 'text-yellow-500'
        }`}>
          {part.risk}
        </span>
      </div>
      <p className="text-sm text-text-secondary bg-bg-tertiary rounded-lg px-3 py-2">
        {part.reason}
      </p>

      {part.status === 'pending' && (
        <div className="flex gap-3 pt-1">
          <button
            onClick={handleReject}
            disabled={loading}
            className="flex-1 px-4 py-2.5 rounded-xl bg-bg-tertiary text-text hover:bg-bg-overlay transition-colors text-sm font-medium disabled:opacity-50"
          >
            拒绝
          </button>
          <button
            onClick={handleApprove}
            disabled={loading}
            className="flex-1 px-4 py-2.5 rounded-xl bg-primary text-white hover:bg-primary-hover transition-colors text-sm font-medium disabled:opacity-50"
          >
            {loading ? '处理中...' : '确认执行'}
          </button>
        </div>
      )}

      {part.status === 'approved' && (
        <div className="text-sm text-green-400 font-medium">✓ 已确认执行</div>
      )}
      {part.status === 'rejected' && (
        <div className="text-sm text-red-400 font-medium">✗ 已拒绝</div>
      )}

      {error && (
        <div className="text-sm text-red-400">{error}</div>
      )}
    </div>
  );
};
```

- [ ] **Step 2: Commit**

```bash
git add frontend/src/components/part-renderers/InteractivePermissionPart.tsx
git commit -m "feat: add InteractivePermissionPart for inline permission confirmation"
```

---

### Task 6: 新增 InteractiveQuestionPart 组件

**Files:**
- Create: `frontend/src/components/part-renderers/InteractiveQuestionPart.tsx`

- [ ] **Step 1: 创建组件文件**

```typescript
import React, { useState } from 'react';
import { apiClient } from '../../services/apiClient';
import { useChatStore } from '../../stores/chatStore';

interface Props {
  turnId: string;
  partIndex: number;
  part: Extract<Part, { type: 'interactive_question' }>;
}

export const InteractiveQuestionPart: React.FC<Props> = ({ turnId, partIndex, part }) => {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [customAnswer, setCustomAnswer] = useState('');
  const updatePart = useChatStore((s) => s.updatePart);

  const handleSelectOption = async (optionId: string, label: string) => {
    if (loading || part.status !== 'pending') return;
    setLoading(true);
    setError(null);
    try {
      await apiClient.respondQuestion(part.tool_call_id, { type: 'option', id: optionId, label });
      updatePart(turnId, partIndex, (p) => ({
        ...(p as any),
        status: 'answered',
        answer: label,
      }));
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to respond');
    } finally {
      setLoading(false);
    }
  };

  const handleCustomAnswer = async () => {
    if (loading || part.status !== 'pending' || !customAnswer.trim()) return;
    setLoading(true);
    setError(null);
    try {
      await apiClient.respondQuestion(part.tool_call_id, { type: 'custom', value: customAnswer.trim() });
      updatePart(turnId, partIndex, (p) => ({
        ...(p as any),
        status: 'answered',
        answer: customAnswer.trim(),
      }));
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to send');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="my-2 p-4 glass border border-tauri-border rounded-2xl space-y-3">
      <div className="text-sm font-medium text-text">
        {part.question}
      </div>

      {part.status === 'pending' && (
        <div className="space-y-2">
          {part.options.map((opt) => (
            <button
              key={opt.id}
              onClick={() => handleSelectOption(opt.id, opt.label)}
              disabled={loading}
              className={`w-full text-left px-4 py-3 rounded-xl text-sm transition-colors disabled:opacity-50 ${
                part.recommended === opt.id
                  ? 'bg-primary/20 text-primary border border-primary/30 hover:bg-primary/30'
                  : 'bg-bg-tertiary text-text hover:bg-bg-overlay border border-transparent'
              }`}
            >
              <div className="flex items-center gap-2">
                {part.recommended === opt.id && (
                  <svg className="w-4 h-4 flex-shrink-0" fill="currentColor" viewBox="0 0 20 20">
                    <path fillRule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clipRule="evenodd" />
                  </svg>
                )}
                <span className="font-medium">{opt.label}</span>
              </div>
              {opt.description && (
                <div className="text-xs text-text-muted mt-1 ml-6">{opt.description}</div>
              )}
            </button>
          ))}

          {part.allow_custom && (
            <div className="flex gap-2 pt-1">
              <input
                type="text"
                value={customAnswer}
                onChange={(e) => setCustomAnswer(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') {
                    e.preventDefault();
                    handleCustomAnswer();
                  }
                }}
                placeholder="自定义回答..."
                disabled={loading}
                className="flex-1 bg-bg-tertiary text-text text-sm rounded-xl px-4 py-2.5 border border-tauri-border focus:outline-none focus:border-primary disabled:opacity-50"
              />
              <button
                onClick={handleCustomAnswer}
                disabled={!customAnswer.trim() || loading}
                className="px-4 py-2.5 rounded-xl bg-primary text-white hover:bg-primary-hover transition-colors text-sm font-medium disabled:opacity-50 disabled:cursor-not-allowed"
              >
                发送
              </button>
            </div>
          )}
        </div>
      )}

      {part.status === 'answered' && (
        <div className="text-sm text-green-400 font-medium">
          ✓ 已回答: {part.answer}
        </div>
      )}

      {error && (
        <div className="text-sm text-red-400">{error}</div>
      )}
    </div>
  );
};
```

- [ ] **Step 2: Commit**

```bash
git add frontend/src/components/part-renderers/InteractiveQuestionPart.tsx
git commit -m "feat: add InteractiveQuestionPart for inline question answering"
```

---

### Task 7: Part 渲染分发逻辑更新

**Files:**
- Modify: `frontend/src/components/part-renderers/index.ts`（或同等导出文件）
- Modify: `frontend/src/components/chat/TurnGroup.tsx` 或 `MessageBubble.tsx`

- [ ] **Step 1: 更新 part-renderers 导出**

在 `frontend/src/components/part-renderers/index.ts` 中：

```typescript
export { InteractivePermissionPart } from './InteractivePermissionPart';
export { InteractiveQuestionPart } from './InteractiveQuestionPart';
```

- [ ] **Step 2: 在 TurnGroup/MessageBubble 的 part 渲染分发中新增分支**

找到渲染 part 的 switch 或条件分支（可能在 `MessageBubble.tsx` 或 `TurnGroup.tsx` 中），新增：

```typescript
import { InteractivePermissionPart, InteractiveQuestionPart } from '../part-renderers';

// 在 part 渲染逻辑中：
switch (part.type) {
  // ... 现有分支
  case 'interactive_permission':
    return <InteractivePermissionPart turnId={turnId} partIndex={index} part={part} />;
  case 'interactive_question':
    return <InteractiveQuestionPart turnId={turnId} partIndex={index} part={part} />;
}
```

注意：`turnId` 和 `index` 需要从父组件传入。

- [ ] **Step 3: Commit**

```bash
git add frontend/src/components/part-renderers/index.ts
git add frontend/src/components/chat/TurnGroup.tsx  # 或 MessageBubble.tsx
git commit -m "feat: wire up interactive parts in message rendering pipeline"
```

---

### Task 8: 移除 PermissionDialog 弹窗

**Files:**
- Modify: `frontend/src/components/layout/AppLayout.tsx`
- Delete: `frontend/src/components/PermissionDialog.tsx`

- [ ] **Step 1: 从 AppLayout.tsx 中移除 PermissionDialog 引用和渲染**

```typescript
// 移除 import
import { PermissionDialog } from '../PermissionDialog';

// 移除 JSX 中的 <PermissionDialog />
```

- [ ] **Step 2: 删除 PermissionDialog.tsx**

```bash
rm frontend/src/components/PermissionDialog.tsx
```

- [ ] **Step 3: Commit**

```bash
git add frontend/src/components/layout/AppLayout.tsx
git rm frontend/src/components/PermissionDialog.tsx
git commit -m "refactor: remove PermissionDialog popup, use inline interaction instead"
```

---

### Task 9: 编译与集成测试

**Files:**
- 所有已修改文件

- [ ] **Step 1: 前端 TypeScript 编译检查**

Run: `cd frontend && npx tsc --noEmit`
Expected: 无类型错误

- [ ] **Step 2: Rust 编译检查**

Run: `cargo check`
Expected: 无编译错误

- [ ] **Step 3: 运行 Rust 测试**

Run: `cargo test -p fi-code-core`
Expected: 全部通过（注意排除 pre-existing 失败的测试）

- [ ] **Step 4: 手动验证流程**

1. 启动后端：`cargo run --bin fi-code-server`
2. 启动前端：`cd frontend && npm run dev`
3. 发送一条触发 `ask_for_question` 的消息（如"问我三个问题"）
4. 验证：问题以交互式组件形式出现在消息流中，而非弹窗
5. 选择一个选项，验证答案能正确送达后端，组件状态变为"已回答"

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: inline interaction for permission and question asks"
```

---

## Self-Review Checklist

**1. Spec coverage:**
- ✅ Low 风险自动放行 → Task 1
- ✅ write/edit 标记为 High → Task 1
- ✅ 信息流内联展示 → Task 4-7
- ✅ 视觉风格一致 → Task 5-6（使用 glass / primary / bg-bg-tertiary 等现有样式）
- ✅ 错误处理 → Task 5-6 中的 error state

**2. Placeholder scan:**
- ✅ 无 TBD/TODO
- ✅ 所有步骤包含实际代码
- ✅ 无模糊描述

**3. Type consistency：**
- ✅ `interactive_permission` / `interactive_question` 类型在 Task 2、5、6、7 中一致
- ✅ `updatePart` 签名在 Task 3、5、6 中一致
- ✅ `apiClient.respondPermission` / `respondQuestion` API 与现有接口一致
