# Slash Command Menu Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在 Web 前端输入框中实现 `/` 指令自动补全菜单。

**Architecture:** `AppLayout` 挂载时拉取 `/api/commands` 存入 `uiStore`，`InputBox` 监听输入并在首字符为 `/` 时渲染过滤菜单，支持键盘/鼠标交互。

---

## Task 1: 添加 CommandMeta 类型

**Files:**
- Create: `frontend/src/types/command.ts`

```typescript
export interface CommandMeta {
  name: string;
  description: string;
  args_hint: string | null;
}
```

**Verification:** `cd frontend && npx tsc --noEmit` passes.

---

## Task 2: uiStore 添加 commands 状态

**Files:**
- Modify: `frontend/src/stores/uiStore.ts`

在 `UIState` interface 中添加：
```typescript
commands: CommandMeta[];
setCommands: (commands: CommandMeta[]) => void;
```

在 store 初始值中添加：
```typescript
commands: [],
setCommands: (commands) => set({ commands }),
```

**Verification:** TypeScript check passes.

---

## Task 3: AppLayout 挂载时拉取指令列表

**Files:**
- Modify: `frontend/src/components/layout/AppLayout.tsx`

在现有 `useEffect` 旁新增：
```typescript
import { CommandMeta } from '../../types/command';

// ...

  // 拉取可用指令列表
  useEffect(() => {
    apiClient
      .get<CommandMeta[]>('/api/commands')
      .then((cmds) => setCommands(cmds))
      .catch((err) => console.warn('[AppLayout] Failed to load commands:', err));
  }, [setCommands]);
```

**Verification:** TypeScript check passes.

---

## Task 4: InputBox 实现 Slash 菜单

**Files:**
- Modify: `frontend/src/components/chat/InputBox.tsx`

核心逻辑：
1. 从 `uiStore` 读取 `commands`
2. `useState` 管理 `showMenu`, `highlightIndex`
3. `onChange` 检测首字符是否为 `/`，提取过滤文本
4. 过滤：`commands.filter(c => c.name.startsWith(filterText))`
5. `onKeyDown` 处理 ↑↓ Enter Tab Esc
6. 菜单绝对定位在 textarea 上方
7. 鼠标点击选中

**Verification:**
- TypeScript check passes
- `npm run build` succeeds

---

## Task 5: 集成验证

- 启动 `fi-code-cli -W <port>`
- 刷新页面
- 在输入框输入 `/`，确认菜单弹出
- 输入 `/mo`，确认过滤为 `models`
- ↑↓ 切换高亮，Enter 填充，Esc 关闭
- 点击菜单项填充
