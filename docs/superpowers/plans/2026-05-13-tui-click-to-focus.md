# TUI 鼠标点击切换焦点实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现鼠标左键点击 TUI 组件时自动切换焦点。

**Architecture:** 在 `TuiApp` 中存储各组件的屏幕区域，收到鼠标左键按下时通过 hit-test 检测点击位置所属组件，更新焦点后正常分发事件。

**Tech Stack:** Rust, ratatui, crossterm

---

### Task 1: 新增 ComponentAreas 结构体并存储区域

**Files:**
- Modify: `crates/tui/src/app.rs:118-205`（TuiApp 字段定义和 new 方法）
- Modify: `crates/tui/src/app.rs:291-416`（draw 方法）

- [ ] **Step 1: 在 TuiApp 中新增 ComponentAreas 字段**

在 `TuiApp` 结构体中（`focus: FocusArea` 字段附近）添加：

```rust
/// 最近一次 draw 时各组件的屏幕区域，用于鼠标 hit-test。
component_areas: ComponentAreas,
```

在 `TuiApp` 的 `new()` 方法中初始化：

```rust
component_areas: ComponentAreas::default(),
```

- [ ] **Step 2: 定义 ComponentAreas 结构体**

在 `crates/tui/src/app.rs` 中 `TuiApp` 结构体之前添加：

```rust
/// 各组件在屏幕上的区域快照，由 draw() 方法在每次渲染后更新。
#[derive(Default)]
struct ComponentAreas {
    left_drawer: Option<Rect>,
    main: Rect,
    input: Rect,
    right_drawer: Rect,
    log_window: Option<Rect>,
    overlay: Option<Rect>,
}
```

- [ ] **Step 3: 在 draw() 结束时保存各组件区域**

在 `draw()` 方法末尾（`question_dialog` 渲染之后，`}` 之前）添加：

```rust
self.component_areas = ComponentAreas {
    left_drawer: areas.left_drawer,
    main: messages_area,
    input: input_area,
    right_drawer: areas.right_drawer,
    log_window: areas.log_window,
    overlay: areas.overlay,
};
```

- [ ] **Step 4: 编译确认无错误**

Run: `cargo check -p fi-code-core`
Expected: 编译通过

- [ ] **Step 5: Commit**

```bash
git add crates/tui/src/app.rs
git commit -m "feat(tui): add ComponentAreas struct to store component screen regions"
```

---

### Task 2: 实现 hit_test 焦点检测

**Files:**
- Modify: `crates/tui/src/app.rs:416-580`（在 route_event 之前添加 hit_test 方法）

- [ ] **Step 1: 新增 hit_test 方法**

在 `TuiApp` impl 块中（`draw` 方法之后、`route_event` 之前）添加：

```rust
/// 根据鼠标坐标检测点击了哪个焦点区域。
/// 按 Z-order 从高到低检测：overlay > dropdown > log_window > drawers > input > main。
fn hit_test(&self, column: u16, row: u16) -> Option<FocusArea> {
    let areas = &self.component_areas;

    // 辅助函数：检查点是否在 Rect 内
    let contains = |rect: &Rect, x: u16, y: u16| -> bool {
        x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height
    };

    // 1. 窄屏覆盖层（最高优先级）
    if let Some(overlay) = areas.overlay {
        if contains(&overlay, column, row) {
            return Some(FocusArea::LeftDrawer);
        }
    }

    // 2. Input 下拉菜单
    if let Some(dropdown) = self.input.dropdown_area() {
        if contains(&dropdown, column, row) {
            return Some(FocusArea::Input);
        }
    }

    // 3. LogWindow（不切换焦点，但消费事件）
    if let Some(log) = areas.log_window {
        if contains(&log, column, row) {
            return None;
        }
    }

    // 4. LeftDrawer
    if let Some(left) = areas.left_drawer {
        if contains(&left, column, row) {
            return Some(FocusArea::LeftDrawer);
        }
    }

    // 5. RightDrawer
    if contains(&areas.right_drawer, column, row) {
        return Some(FocusArea::RightDrawer);
    }

    // 6. Input
    if contains(&areas.input, column, row) {
        return Some(FocusArea::Input);
    }

    // 7. Main
    if contains(&areas.main, column, row) {
        return Some(FocusArea::Main);
    }

    None
}
```

> 注意：`self.input.dropdown_area()` 需要确认 Input 组件是否有公开方法返回 dropdown 的 Rect。如果还没有，需要在 Input 组件中添加。

- [ ] **Step 2: 确认 Input 有 dropdown_area 访问方法**

检查 `crates/tui/src/components/input.rs` 中是否有 `dropdown_area` 的 getter。如果没有，添加：

```rust
pub fn dropdown_area(&self) -> Option<Rect> {
    self.dropdown_area
}
```

- [ ] **Step 3: 编译确认无错误**

Run: `cargo check -p fi-code-core`
Expected: 编译通过

- [ ] **Step 4: Commit**

```bash
git add crates/tui/src/app.rs crates/tui/src/components/input.rs
git commit -m "feat(tui): add hit_test method for mouse coordinate to focus area mapping"
```

---

### Task 3: 在 route_event 中处理鼠标左键按下切换焦点

**Files:**
- Modify: `crates/tui/src/app.rs:581-677`（route_event 方法）

- [ ] **Step 1: 修改 Event::Mouse 分支，增加 Down 事件处理**

将现有的 `Event::Mouse` 分支：

```rust
Event::Mouse(mouse) => {
    // 鼠标事件按当前焦点分发给对应组件
    self.dispatch_event(Event::Mouse(mouse)).await;
}
```

替换为：

```rust
Event::Mouse(mouse) => {
    use crossterm::event::MouseEventKind;

    // 鼠标左键按下时，检测点击位置并切换焦点
    if let MouseEventKind::Down(crossterm::event::MouseButton::Left) = mouse.kind {
        if let Some(new_focus) = self.hit_test(mouse.column, mouse.row) {
            if new_focus != self.focus {
                log_debug!(
                    "[Client] Focus switched by mouse click | {:?} -> {:?}",
                    self.focus,
                    new_focus
                );
                self.focus = new_focus;
                self.dirty = true;
            }
        }
    }

    // 继续将事件分发给（可能已切换的）焦点组件
    self.dispatch_event(Event::Mouse(mouse)).await;
}
```

- [ ] **Step 2: 编译确认无错误**

Run: `cargo check -p fi-code-core`
Expected: 编译通过

- [ ] **Step 3: Commit**

```bash
git add crates/tui/src/app.rs
git commit -m "feat(tui): mouse left-click switches focus to clicked component"
```

---

### Task 4: 编译与测试

- [ ] **Step 1: 编译完整项目**

Run: `cargo build -p fi-code-cli -p fi-code-tui`
Expected: 编译成功

- [ ] **Step 2: 运行现有测试确保未破坏行为**

Run: `cargo test -p fi-code-tests --test bdd --test e2e_cli --test e2e_tui`
Expected: 全部通过（BDD 20/20, E2E CLI 6/6, E2E TUI 3/3）

- [ ] **Step 3: Commit**

```bash
git commit --allow-empty -m "test: verify click-to-focus does not break existing tests"
```

---

## 自审

1. **Spec 覆盖**：设计文档中的 4 个部分（存储区域、hit-test、切换时机、边界情况）均已覆盖。
2. **Placeholder scan**：无 TBD/TODO，所有步骤包含具体代码。
3. **类型一致性**：`ComponentAreas` 在 Step 1 定义，`hit_test` 在 Step 2 使用，`focus` 字段在 Step 3 修改，类型一致。
