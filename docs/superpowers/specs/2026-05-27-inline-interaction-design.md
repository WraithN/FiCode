# 信息流内联交互设计文档

## 背景

当前 `PermissionAsk`（权限确认）和 `QuestionAsk`（问题询问）采用弹窗（`PermissionDialog`）形式展示，打断用户阅读聊天流的体验。本设计将其改为在聊天信息流中内联展示交互组件，同时将所有 Low 风险工具改为自动放行（Allow），减少不必要的确认。

## 目标

1. **消除弹窗**：`PermissionAsk` 和 `QuestionAsk` 不再使用全局弹窗，而是在当前 turn 的消息流中内联渲染。
2. **Low 风险自动放行**：除明确的高风险操作外，所有 Low 风险工具静默执行，不打扰用户。
3. **视觉一致**：交互组件采用与当前主题一致的玻璃拟态 + 青蓝紫渐变风格。

## 方案概览

### 后端改动

#### 1. 风险等级调整 (`permission.rs`)

修改 `PermissionAction::match_action()` 的默认分支和 `write`/`edit` 的风险等级：

```rust
// 现有 Low 风险自动放行工具（已 Allow，无需改动）
"read" | "read_file" | "grep" | "ask_for_question" → Allow + Low

// 提升 write/edit 为 High 风险（需要用户确认）
"write" | "edit" → Ask + High

// bash（非危险命令）保持 Ask + High
"bash"（安全命令）→ Ask + High

// 其他所有工具默认改为 Allow + Low
其他工具 → Allow + Low
```

**影响范围**：`git_status`, `git_log`, `git_diff`, `git_add`, `git_commit`, `glob`, `web_fetch`, `use_skill` 等工具将不再触发权限确认，直接静默执行。

#### 2. SSE 事件保留

`SseEvent::PermissionAsk` 和 `SseEvent::QuestionAsk` 继续保留，仅前端渲染方式改变。这样 TUI/CLI 模式不受此改动影响。

### 前端改动

#### 1. 新增 Part 类型

在 `frontend/src/types/part.ts` 中新增两种交互式 part：

```typescript
| { 
    type: 'interactive_permission'; 
    tool_call_id: string; 
    tool_name: string; 
    risk: string; 
    reason: string; 
    status: 'pending' | 'approved' | 'rejected' 
  }
| { 
    type: 'interactive_question'; 
    tool_call_id: string; 
    question: string; 
    options: { id: string; label: string; description?: string }[]; 
    recommended?: string;
    allow_custom: boolean;
    status: 'pending' | 'answered'; 
    answer?: string 
  }
```

#### 2. 移除弹窗渲染

- 从 `AppLayout.tsx` 中移除 `<PermissionDialog />` 组件引用
- `PermissionDialog.tsx` 文件可删除或保留为备用

#### 3. SSE 事件处理改为 appendPart

修改 `frontend/src/hooks/useChatStream.ts` 中的 `handleSseEvent`：

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

#### 4. 新增 Part 渲染组件

新增 `frontend/src/components/part-renderers/InteractivePermissionPart.tsx`：
- 展示工具名、风险等级（High 用橙色标签）、原因说明
- 两个按钮：拒绝（bg-bg-tertiary）/ 确认执行（bg-primary gradient）
- 点击后调用 `apiClient.respondPermission()`
- 调用成功后更新 part 状态为 `approved` 或 `rejected`，按钮置灰并显示结果

新增 `frontend/src/components/part-renderers/InteractiveQuestionPart.tsx`：
- 展示问题文本
- 选项按钮列表：推荐选项高亮（bg-primary/20 + 边框）
- 自定义输入框（如果 `allow_custom` 为 true）
- 点击后调用 `apiClient.respondQuestion()`
- 调用成功后更新状态为 `answered`，显示已选答案

#### 5. Part 路由更新

在 `TurnGroup` 或 `MessageBubble` 的 part 渲染分发逻辑中，新增对 `interactive_permission` 和 `interactive_question` 的分支：

```typescript
switch (part.type) {
  case 'interactive_permission':
    return <InteractivePermissionPart part={part} />;
  case 'interactive_question':
    return <InteractiveQuestionPart part={part} />;
  // ... 其他类型
}
```

#### 6. chatStore 支持 part 状态更新

在 `chatStore.ts` 中新增 `updatePart` 方法，用于更新特定 turn 中特定 part 的状态：

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

组件内部通过 `updatePart` 更新自身状态。

## 数据流

```
后端工具执行
  → PermissionAction::match_action() 判断风险
    → Low 风险: 直接执行，不发 SSE
    → High 风险: 发送 PermissionAsk SSE
    → QuestionAsk: 发送 QuestionAsk SSE

前端 SSE 接收
  → useChatStream.handleSseEvent()
    → permission_ask: appendPart(interactive_permission)
    → question_ask: appendPart(interactive_question)

前端渲染
  → TurnGroup → MessageBubble → Part 分发
    → interactive_permission → InteractivePermissionPart (内联按钮)
    → interactive_question → InteractiveQuestionPart (内联选项)

用户交互
  → 点击按钮 → apiClient.respondPermission/Question()
    → 后端 respond_permission/respond_question() → 通知等待中的 wait
    → 组件 updatePart() 更新状态为 approved/rejected/answered
```

## 错误处理

1. **API 调用失败**：组件内捕获错误，显示红色提示文本，保持 `pending` 状态，允许用户重试。
2. **超时**：后端 `wait_permission_response`（30s）和 `wait_question_response`（60s）超时后，返回 `ToolError` part，前端正常展示错误信息。
3. **重复点击**：按钮点击后立即置灰（loading 状态），防止重复提交。

## 测试策略

1. **单元测试**：
   - `permission.rs`: 验证 `match_action` 对各类工具的返回结果（write/edit 为 High，其他 Low 为 Allow）
   - `apiClient.ts`: 验证 respondPermission/respondQuestion 正常返回

2. **E2E/BDD**：
   - 覆盖 `ask_for_question` 信息流交互流程
   - 覆盖权限确认内联交互流程

## 相关文件

- `crates/core/src/permission/permission.rs`
- `frontend/src/types/part.ts`
- `frontend/src/hooks/useChatStream.ts`
- `frontend/src/stores/chatStore.ts`
- `frontend/src/components/layout/AppLayout.tsx`
- `frontend/src/components/part-renderers/InteractivePermissionPart.tsx` (新增)
- `frontend/src/components/part-renderers/InteractiveQuestionPart.tsx` (新增)
- `frontend/src/components/PermissionDialog.tsx` (删除或弃用)
