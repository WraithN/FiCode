# Desktop 前端对齐 TUI 设计规格书

> 设计日期：2026-05-18
> 状态：待实现

---

## 1. 背景与目标

### 1.1 背景

当前 fi-code Desktop 前端（`frontend/`）是一个早期简化实现，与 TUI（`crates/tui/`）在数据模型、通信协议、功能完整性上存在显著差距：

- **数据模型**：Desktop 使用简单的 `{ role, content }` 消息列表；TUI 使用 `Part`-based 渲染 + `Turn` 对话回合
- **通信协议**：Desktop 的 SSE 解析器将后端完整事件简化为 `{ type: 'content'|'done'|'error' }`；TUI 完整解析所有 `SseEvent` 变体
- **功能缺失**：Desktop 无 Agent 切换、无 Part 渲染、无 TaskProgress 展示、无 Token 使用量显示
- **主题系统**：Desktop 只有 3 个硬编码主题，字段与 TUI 的 `ThemePreset` 不兼容

### 1.2 设计目标

1. **统一数据模型**：Desktop 完整引入 `Part` / `Turn` / `AgentType` 类型系统，与后端 Rust DTO 完全对应
2. **统一通信协议**：SSE 层完整解析所有事件类型（`Message` / `Part` / `AgentInfo` / `TaskProgress` / `Done` / `Error`）
3. **布局对齐**：Desktop 布局结构与 TUI 对齐（左文件抽屉 + 中间聊天区 + 右历史抽屉 + 底部状态栏）
4. **功能对等**：支持 Agent 切换、Part 渲染、Token 使用量显示、日志浮窗
5. **主题共享**：Desktop 直接复用 TUI 的 `preset_themes.json`，字段完全对齐
6. **彻底重写**：一次性替换类型定义、状态管理、API 层、渲染层，保留少量 UI 外壳

---

## 2. 核心数据结构

### 2.1 Part 类型系统

```typescript
// frontend/src/types/part.ts

export type Part =
  | { type: 'text'; text: string }
  | { type: 'tool_use'; id: string; name: string; arguments: Record<string, unknown> }
  | { type: 'tool_result'; tool_call_id: string; content: string; duration_ms?: number }
  | { type: 'tool_error'; tool_call_id: string; content: string; error_message: string }
  | { type: 'thinking'; content: string }
  | { type: 'code_block'; language: string; code: string }
  | { type: 'image'; url: string; alt?: string }
  | { type: 'usage'; prompt_tokens: number; completion_tokens: number }
  | { type: 'wave_marker'; wave_id: string; turn: number };
```

### 2.2 SSE 事件类型

```typescript
// frontend/src/types/sse.ts

export interface TaskProgressItem {
  id: string;
  name: string;
  status: string;
}

export type SseEvent =
  | { type: 'message'; content: string }
  | { type: 'part'; part: Part }
  | { type: 'agent_info'; agent_type: AgentType; agent_name: string }
  | { type: 'task_progress'; plan_id: string; tasks: TaskProgressItem[] }
  | { type: 'done'; session_id: string }
  | { type: 'error'; message: string };
```

### 2.3 Turn 结构（对话回合）

```typescript
// frontend/src/types/turn.ts

export interface Turn {
  id: string;
  userMessage: string;
  parts: Part[];
  isComplete: boolean;
  timestamp: number;
}
```

### 2.4 Agent 类型

```typescript
// frontend/src/types/agent.ts

export type AgentType = 'build' | 'plan';
```

---

## 3. 状态管理架构

按 domain 拆分为 4 个独立 Zustand store，替代当前的单一 `appStore`。

### 3.1 Store 拆分

```typescript
// stores/connectionStore.ts
interface ConnectionState {
  mode: 'standalone' | 'remote';
  connectionStatus: 'connecting' | 'connected' | 'error';
  serverUrl: string;
  connectionError: string | null;
}

// stores/sessionStore.ts
interface SessionState {
  currentSessionId: string | null;
  sessions: SessionInfo[];
}

// stores/chatStore.ts —— 核心重构点
interface ChatState {
  turns: Turn[];
  isGenerating: boolean;
  currentAgent: AgentType;
  
  startTurn: (userMessage: string) => string;  // 返回 turnId
  appendPart: (turnId: string, part: Part) => void;
  completeTurn: (turnId: string) => void;
  setAgent: (agent: AgentType) => void;
  setIsGenerating: (generating: boolean) => void;
  clearTurns: () => void;
  getCurrentTurnId: () => string | null;
}

// stores/uiStore.ts
interface UIState {
  leftDrawerOpen: boolean;
  rightDrawerOpen: boolean;
  logOpen: boolean;
  themeName: string;
  providers: ProviderItem[];
  currentModel: string;
}
```

### 3.2 ChatStore 核心逻辑

- **Turn 创建时机**：用户点击发送时立即创建 Turn，`isComplete` 为 `false`
- **Part 追加**：SSE `Part` 事件直接 `appendPart`；SSE `Message` 事件转换为 `Part::Text` 追加
- **Turn 完成**：收到 `SseEvent::Done` 时 `completeTurn` 设为 `true`
- **Agent 持久化**：`currentAgent` 作为 `ChatRequest` 的 `agent` 字段发送

---

## 4. SSE 通信层

### 4.1 完整事件解析

`ApiClient.chatStream` 从简化映射改为完整 `AsyncGenerator<SseEvent>`：

```typescript
async *chatStream(
  sessionId: string | null,
  message: string,
  agent: AgentType = 'build'
): AsyncGenerator<SseEvent, string, unknown> {
  // 发送请求，解析 SSE 流
  // 支持多行 data: 合并（与 TUI client.rs 逻辑对齐）
  // yield 完整 SseEvent，遇到 Done 时返回 session_id
}
```

### 4.2 事件分发

`useChatStream` hook 作为分发中心：

| SSE 事件 | 处理动作 |
|----------|----------|
| `message` | 转为 `Part::Text` append 到当前 Turn |
| `part` | 直接 `appendPart` |
| `agent_info` | `chatStore.setAgent()` |
| `task_progress` | 可扩展为 UI 任务进度展示 |
| `done` | `completeTurn()` + `setIsGenerating(false)` |
| `error` | `appendPart({ type: 'tool_error', ... })` 或系统提示 |

---

## 5. 组件架构

### 5.1 布局结构

```
App
├── Header（简化：品牌 + 模型下拉）
├── MainLayout
│   ├── LeftDrawer（文件树，可折叠）
│   ├── ChatArea
│   │   ├── ChatPanel（Turn 列表）
│   │   └── InputBox（输入框）
│   └── RightDrawer（会话历史，可折叠）
├── StatusBar（新增：AGT / Model / Tokens）
└── LogPanel（日志浮窗）
```

### 5.2 Part 渲染注册表

```typescript
// components/part-renderers/registry.tsx

const partRenderers: Record<Part['type'], React.FC<{ part: Part }>> = {
  text: TextPart,
  thinking: ThinkingPart,
  tool_use: ToolUsePart,
  tool_result: ToolResultPart,
  tool_error: ToolErrorPart,
  code_block: CodeBlockPart,
  image: ImagePart,
  usage: UsagePart,
  wave_marker: WaveMarkerPart,
};
```

### 5.3 Agent 切换

Desktop 无键盘快捷键传统，Agent 切换放在 StatusBar：

```typescript
// StatusBar.tsx
<button onClick={() => setAgent(currentAgent === 'build' ? 'plan' : 'build')}>
  AGT: {currentAgent === 'build' ? 'Build' : 'Plan'}
</button>
```

---

## 6. 主题系统

### 6.1 扩展 ThemeColors

新增字段对齐 TUI：
- `bgUserArea` / `bgAiArea`：用户/AI 消息区域背景
- `textPlaceholder`：占位符文字
- `brand`（由 `accent` 重命名）
- `accentHover`：悬停高亮
- `user`：用户标识色
- `selectionBg` / `selectionFg`：选区色彩

### 6.2 共享主题数据源

Desktop 直接复用 `crates/shared/src/preset_themes.json`，通过 `u32ToHex()` 转换：

```typescript
import presetJson from '../../../crates/shared/src/preset_themes.json';

function u32ToHex(u32: number): string {
  return `#${u32.toString(16).padStart(6, '0')}`;
}
```

### 6.3 CSS 变量扩展

新增变量：`--color-bg-user-area`, `--color-bg-ai-area`, `--color-brand`, `--color-user`, `--color-selection-bg` 等。

---

## 7. 废弃清单

| 文件/模块 | 处理方式 | 说明 |
|-----------|----------|------|
| `frontend/src/types/api.ts`（旧 SseEvent/Message） | 删除旧类型，保留通用 API 类型 | 被新 `types/sse.ts`、`types/part.ts`、`types/turn.ts` 替代 |
| `frontend/src/types/events.ts` | 删除 | 旧 AppEvent 模型废弃 |
| `frontend/src/stores/appStore.ts` | 删除 | 被 4 个独立 store 替代 |
| `frontend/src/services/chat.ts` | 重写 | 支持完整 SseEvent 和 agent 参数 |
| `frontend/src/services/client.ts`（chatStream） | 重写 | 完整 SSE 解析 |
| `frontend/src/components/MessageBubble.tsx` | 删除 | 被 `TurnGroup` + `PartRenderer` 替代 |
| `frontend/src/components/ChatPanel.tsx` | 重写 | 支持 Turn 列表 |
| `frontend/src/components/Header.tsx` | 简化 | 状态信息下沉到 StatusBar |
| `frontend/src/components/Sidebar.tsx` | 重命名为 LeftDrawer | 功能扩展为文件树 |
| `frontend/src/components/HistoryDrawer.tsx` | 重构为 RightDrawer | 改为固定侧边栏 |
| `frontend/src/themes/presets/*.ts` | 删除 | 改为从 JSON 加载 |

---

## 8. 测试策略

### 8.1 单元测试

- `types/`：Part、SseEvent、Turn 的类型守卫和转换函数
- `stores/chatStore`：Turn 创建、Part 追加、完成状态转换
- `services/apiClient`：SSE 流解析（使用 mock ReadableStream）

### 8.2 集成测试

- `useChatStream` hook 的完整事件处理流程
- `PartRenderer` 注册表对所有 Part 类型的渲染

### 8.3 E2E 测试

- Desktop 构建后验证与后端完整协议通信
- Agent 切换功能端到端验证

---

## 9. 风险与回滚方案

| 风险 | 缓解措施 |
|------|----------|
| 重写范围过大导致构建失败 | 分阶段提交，每阶段保持可编译 |
| 主题 JSON 加载路径在构建后失效 | 使用 Vite 的 `?raw` 或 `?json` 导入，确保打包后可用 |
| SSE 多行 data 解析边界情况 | 直接移植 TUI client.rs 的已验证逻辑 |
| Part 类型与后端不完全对齐 | 使用 TypeScript 严格模式 + 运行时类型守卫 |

**回滚**：所有修改在 `frontend/` 目录内，可通过 `git checkout frontend/` 快速回滚到旧实现。
