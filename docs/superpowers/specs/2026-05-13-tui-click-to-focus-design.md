# TUI 鼠标点击切换焦点设计

## 背景

当前 TUI 的 4 个焦点区域（`Main`、`Input`、`LeftDrawer`、`RightDrawer`）只能通过 `Tab`/`Shift+Tab` 循环切换。鼠标事件（包括滚轮、点击）只分发给**当前获得焦点的组件**，没有区域检测。用户无法通过鼠标左键点击某个组件来将焦点切换到该组件。

## 目标

鼠标左键点击某个组件区域时，焦点自动切换到该组件。切换后，后续的鼠标事件（如滚轮）正常分发给新焦点组件。

## 设计方案

采用 **TuiApp 统一存储区域并 hit-test** 方案。

### 1. 存储组件区域

在 `TuiApp` 中新增 `component_areas` 字段，在 `draw()` 每次渲染结束后保存各组件的实际屏幕 `Rect`：

```rust
struct ComponentAreas {
    left_drawer: Option<Rect>,   // 仅 LeftOpen 时有值
    main: Rect,                  // Chat 消息区（main 的上半部分）
    input: Rect,                 // 输入框（main 的下半部分）
    right_drawer: Rect,
    log_window: Option<Rect>,    // 仅可见时有值
    overlay: Option<Rect>,       // 窄屏模式下左侧覆盖层
}
```

> 注：`main` 和 `input` 由 `LayoutManager::split_main()` 拆分得到，`left_drawer` 和 `overlay` 互斥（窄屏模式下 drawer 以 overlay 形式呈现）。

### 2. Hit-test 函数

新增 `hit_test(column: u16, row: u16) -> Option<FocusArea>`，按 **Z-order 从高到低** 检测：

1. **Overlay**（窄屏左侧覆盖层）→ `LeftDrawer`
2. **Dropdown**（Input 的下拉菜单）→ `Input`
3. **LogWindow**（日志浮层）→ 不切换焦点（保持现有行为：LogWindow 不是 `FocusArea` 成员，点击时只消费事件不抢焦点）
4. **LeftDrawer**（正常模式左侧抽屉）→ `LeftDrawer`
5. **RightDrawer**（右侧会话列表）→ `RightDrawer`
6. **Input**（底部输入框）→ `Input`
7. **Main**（聊天主区域）→ `Main`

检测逻辑：
```rust
x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height
```

### 3. 焦点切换时机

在 `route_event()` 的 `Event::Mouse` 分支中，对 `MouseEventKind::Down` 做拦截：

```rust
Event::Mouse(mouse) => {
    if let MouseEventKind::Down(_) = mouse.kind {
        if let Some(new_focus) = self.hit_test(mouse.column, mouse.row) {
            if new_focus != self.focus {
                self.focus = new_focus;
                self.dirty = true;
            }
        }
    }
    // 继续将事件分发给（可能已切换的）焦点组件
    self.dispatch_event(Event::Mouse(mouse)).await;
}
```

### 4. 边界情况

- **点击空白区域**（不在任何组件内）：`hit_test` 返回 `None`，焦点不变
- **点击 LogWindow**：事件被 `dispatch_event` 中的 LogWindow 拦截处理（保持现有滚动逻辑），焦点不变
- **模态框可见时**（API Key 对话框、QuestionDialog）：模态框直接消费键盘事件，但鼠标点击事件仍走 `route_event`。点击模态框外部区域时，按上述逻辑切换焦点；点击模态框内部时，若模态框没有占据独立的 `FocusArea`，焦点不变
- **鼠标滚轮**：不属于 `Down` 事件，不走 hit-test，直接分发给当前焦点组件（已有行为）

## 测试策略

- E2E 测试：在 TUI E2E 测试中模拟鼠标点击不同区域，验证焦点状态变化
- BDD 测试：补充 Gherkin 场景描述点击切换焦点的行为

## 不涉及的改动

- 不改 `Component` trait 定义
- 不改动各组件内部的 `handle_event` 逻辑
- 不引入新的第三方库
