# TUI Hover 焦点切换与滚动条设计

> 日期：2026-05-13
> 方案：A（最小改动）

---

## 背景

当前 TUI 的焦点切换只在鼠标左键点击（`MouseEventKind::Down`）时触发。用户希望鼠标 hover（`MouseEventKind::Moved`）到任意区域时焦点自动跟随。

此外，`LeftDrawer`（文件列表）、`RightDrawer`（右侧边栏）目前不支持滚动，内容超出区域会被截断；`LogWindow` 支持键盘滚动但没有滚动条视觉指示。用户要求这三个组件都加上滚动条，并支持鼠标滚轮滚动。

---

## 目标

1. **全局 hover 焦点切换**：鼠标移动到任意组件区域时，焦点自动切换到该组件。
2. **滚动条**：`LeftDrawer`、`RightDrawer`、`LogWindow` 在内容超出可视区域时显示 scrollbar。
3. **鼠标滚轮滚动**：上述三个组件支持鼠标滚轮（`ScrollUp`/`ScrollDown`）滚动，每次 3 行（与 `Chat` 组件保持一致）。
4. **键盘焦点循环**：`Tab`/`Shift+Tab` 焦点循环要包含 `LogWindow`。

---

## 设计细节

### 1. FocusArea 扩展

在 `crates/core/src/tui/event.rs` 的 `FocusArea` 枚举中添加 `LogWindow` 变体：

```rust
pub enum FocusArea {
    Main,
    Input,
    LeftDrawer,
    RightDrawer,
    LogWindow, // 新增
}
```

### 2. Hover 焦点切换

在 `crates/core/src/tui/app.rs` 的 `route_event` 中，对 `MouseEventKind::Moved` 执行与 `Down` 相同的 `hit_test` 逻辑：

```rust
Event::Mouse(mouse) => {
    match mouse.kind {
        MouseEventKind::Down(...) | MouseEventKind::Moved => {
            if let Some(new_focus) = self.hit_test(mouse.column, mouse.row) {
                if new_focus != self.focus {
                    self.focus = new_focus;
                    self.dirty = true;
                }
            }
        }
        _ => {}
    }
    self.dispatch_event(Event::Mouse(mouse)).await;
}
```

**`hit_test` 修改**：LogWindow 分支从返回 `None` 改为返回 `Some(FocusArea::LogWindow)`。

### 3. 滚动条渲染

使用 ratatui 内置 `Scrollbar` widget。在每个组件的 `draw` 方法中，在内容区域右侧绘制 scrollbar。

**Scrollbar 样式**：
- 颜色：`theme.border`
- 只在 `content_lines > viewport_height` 时显示
- thumb 符号：`▐`（或 ratatui 默认）

**滚动状态计算**（以 `LeftDrawer` 为例）：
- `content_length = self.files.len()`
- `viewport_height = inner.height as usize`
- `scroll_offset` 控制当前顶部显示的行索引
- `ScrollbarState::default().content_length(content_length).position(scroll_offset).viewport_content_length(viewport_height)`

### 4. 各组件滚动实现

#### LeftDrawer

新增字段：
```rust
scroll_offset: usize
```

`draw` 修改：
- 使用 `self.files.iter().skip(self.scroll_offset).take(viewport_height)` 渲染可见项
- 在右侧绘制 `Scrollbar`
- 选中项反色高亮逻辑不变（`selected_index` 是全局索引，不受 `scroll_offset` 影响）

`handle_event` 新增：
```rust
MouseEventKind::ScrollUp => { self.scroll_up(3); }
MouseEventKind::ScrollDown => { self.scroll_down(3); }
```

辅助方法：
```rust
fn scroll_up(&mut self, delta: usize) {
    self.scroll_offset = self.scroll_offset.saturating_sub(delta);
}
fn scroll_down(&mut self, delta: usize) {
    let max = self.files.len().saturating_sub(1);
    self.scroll_offset = (self.scroll_offset + delta).min(max);
}
```

**边界处理**：当 `selected_index` 滚出可视区域时，是否需要自动滚动？
- 决策：保持简单。键盘 `Up`/`Down` 只移动 `selected_index`，不联动 `scroll_offset`。如果用户需要看到选中项，可以手动滚动或后续优化。

#### RightDrawer

与 `LeftDrawer` 完全相同的模式：
- 新增 `scroll_offset: usize`
- `draw` 时用 `skip`/`take` 截断
- 右侧绘制 `Scrollbar`
- `handle_event` 处理 `ScrollUp`/`ScrollDown`

#### LogWindow

已有 `scroll_offset`，只需：
- `draw` 时添加 `Scrollbar` widget
- `handle_event` 添加 `ScrollUp`/`ScrollDown` 处理（与键盘 `Up`/`Down` 相同逻辑，每次 3 行）

注意：LogWindow 的 `scroll_offset` 语义是 **从底部向上的偏移**（0 = 底部）。Scrollbar 的 `position` 需要反转计算：
- `content_length = self.lines.len()`
- `position = content_length.saturating_sub(visible_height).saturating_sub(self.scroll_offset)`

### 5. 焦点循环（cycle_focus）

在 `app.rs` 的 `cycle_focus` 中，根据当前 `PanelState` 将 `LogWindow` 加入可用区域列表。

由于 `LogWindow` 是浮窗（visible 时才存在），只在 `log_window.is_visible()` 时加入循环。

```rust
let mut areas = vec![FocusArea::Main, FocusArea::Input, FocusArea::RightDrawer];
if self.layout.panel == PanelState::LeftOpen {
    areas.insert(0, FocusArea::LeftDrawer);
}
if self.log_window.is_visible() {
    // 插入到合适位置，比如在 Input 之后
    areas.insert(areas.iter().position(|a| a == &FocusArea::Input).unwrap_or(0) + 1, FocusArea::LogWindow);
}
```

### 6. dispatch_event 扩展

在 `app.rs` 的 `dispatch_event` 中，添加 `FocusArea::LogWindow` 分支，将事件分发给 `self.log_window.handle_event`。

---

## 文件改动清单

| 文件 | 改动类型 | 说明 |
|------|----------|------|
| `crates/core/src/tui/event.rs` | 修改 | `FocusArea` 添加 `LogWindow` |
| `crates/core/src/tui/app.rs` | 修改 | hover 切焦点、LogWindow hit_test、cycle_focus、dispatch_event |
| `crates/core/src/tui/components/left_drawer.rs` | 修改 | 添加 scroll_offset、scrollbar、滚轮处理 |
| `crates/core/src/tui/components/right_drawer.rs` | 修改 | 添加 scroll_offset、scrollbar、滚轮处理 |
| `crates/core/src/tui/components/log_window.rs` | 修改 | 添加 scrollbar、滚轮处理 |

---

## 测试策略

1. **单元测试**：
   - `LeftDrawer::scroll_up` / `scroll_down` 边界测试
   - `RightDrawer::scroll_up` / `scroll_down` 边界测试
   - `LogWindow` 滚轮与键盘滚动一致性测试

2. **BDD 测试**（如需要）：
   - 可通过 Mock 终端事件测试 hover 焦点切换，但当前 BDD 框架主要测试后端 API，TUI 交互在单元测试中覆盖即可。

3. **手动测试**：
   - hover 到各区域验证焦点边框变化（`BorderType::Double`）
   - 滚轮滚动验证内容移动和 scrollbar thumb 位置
   - Tab 焦点循环验证 LogWindow 被包含

---

## 风险与注意事项

1. **`Moved` 事件频率**：crossterm 的 `Moved` 事件在鼠标移动时持续触发。每次触发只修改 `self.dirty = true`（标记重绘），不会立即重绘，实际重绘由主循环的 tick 驱动，性能影响可控。
2. **LogWindow 焦点冲突**：LogWindow 是浮窗，hover 到 LogWindow 上时焦点切换到 LogWindow，此时键盘事件（如 Esc）由 LogWindow 消费。需要确认 Esc 关闭 LogWindow 的逻辑是否仍然有效。
3. **Scrollbar 与内容区域重叠**：Scrollbar 宽度为 1 列，需要确保内容区域的 `inner` 宽度减去 1 列给 scrollbar，避免内容被覆盖。
