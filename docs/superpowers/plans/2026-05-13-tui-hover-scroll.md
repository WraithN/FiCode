# TUI Hover 焦点切换与滚动条实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 TUI 添加全局 hover 焦点切换，并为 LeftDrawer、RightDrawer、LogWindow 添加滚动条与鼠标滚轮支持。

**Architecture:** 最小改动方案。`FocusArea` 新增 `LogWindow`；`app.rs` 在 `MouseEventKind::Moved` 时执行 `hit_test` 切焦点；三个组件各自添加 `scroll_offset`、ratatui `Scrollbar` widget、滚轮事件处理。

**Tech Stack:** Rust, ratatui, crossterm

---

## 文件改动清单

| 文件 | 操作 | 说明 |
|------|------|------|
| `crates/tui/src/event.rs` | 修改 | `FocusArea` 添加 `LogWindow` |
| `crates/tui/src/app.rs` | 修改 | hover 切焦点、hit_test LogWindow、cycle_focus、dispatch_event |
| `crates/tui/src/components/left_drawer.rs` | 修改 | scroll_offset、scrollbar、滚轮 |
| `crates/tui/src/components/right_drawer.rs` | 修改 | scroll_offset、scrollbar、滚轮 |
| `crates/tui/src/components/log_window.rs` | 修改 | scrollbar、滚轮 |

---

### Task 1: FocusArea 添加 LogWindow

**Files:**
- Modify: `crates/tui/src/event.rs:170-175`

- [ ] **Step 1: 修改 FocusArea 枚举**

在 `FocusArea` 枚举末尾添加 `LogWindow` 变体：

```rust
pub enum FocusArea {
    Main,
    Input,
    LeftDrawer,
    RightDrawer,
    LogWindow, // 新增
}
```

- [ ] **Step 2: 编译检查**

Run: `cargo check -p fi-code-core`
Expected: 编译通过，但 `app.rs` 中 `match` 会报 `non-exhaustive patterns` 警告/错误（预期，下一步修复）

- [ ] **Step 3: Commit**

```bash
git add crates/tui/src/event.rs
git commit -m "feat(tui): add LogWindow to FocusArea enum"
```

---

### Task 2: app.rs 修改 — hover 焦点切换与 LogWindow 支持

**Files:**
- Modify: `crates/tui/src/app.rs`

- [ ] **Step 1: hit_test 修改 — LogWindow 返回焦点**

将 `hit_test` 中 LogWindow 分支从 `return None` 改为 `return Some(FocusArea::LogWindow)`：

```rust
// 3. LogWindow
if let Some(log) = areas.log_window {
    if contains(&log, column, row) {
        return Some(FocusArea::LogWindow);
    }
}
```

- [ ] **Step 2: route_event 修改 — Moved 时切焦点**

在 `Event::Mouse(mouse)` 处理分支中，将 `MouseEventKind::Moved` 与 `Down` 一并处理：

```rust
Event::Mouse(mouse) => {
    use crossterm::event::MouseEventKind;

    match mouse.kind {
        MouseEventKind::Down(crossterm::event::MouseButton::Left)
        | MouseEventKind::Moved => {
            if let Some(new_focus) = self.hit_test(mouse.column, mouse.row) {
                if new_focus != self.focus {
                    log_debug!(
                        "[Client] Focus switched by mouse | {:?} -> {:?}",
                        self.focus,
                        new_focus
                    );
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

- [ ] **Step 3: cycle_focus 修改 — 包含 LogWindow**

在 `cycle_focus` 中，根据 `log_window.is_visible()` 动态加入 `FocusArea::LogWindow`：

```rust
fn cycle_focus(&mut self, forward: bool) {
    let mut areas = match self.layout.panel {
        PanelState::LeftClosed => {
            vec![FocusArea::Main, FocusArea::Input, FocusArea::RightDrawer]
        }
        PanelState::LeftOpen => {
            vec![
                FocusArea::LeftDrawer,
                FocusArea::Main,
                FocusArea::Input,
                FocusArea::RightDrawer,
            ]
        }
    };

    // LogWindow 可见时插入焦点循环（放在 Input 之后）
    if self.log_window.is_visible() {
        let input_idx = areas.iter().position(|a| a == &FocusArea::Input).unwrap_or(1);
        areas.insert(input_idx + 1, FocusArea::LogWindow);
    }

    let current_idx = areas.iter().position(|a| a == &self.focus).unwrap_or(0);
    let next_idx = if forward {
        (current_idx + 1) % areas.len()
    } else {
        (current_idx + areas.len() - 1) % areas.len()
    };

    self.focus = areas[next_idx];
}
```

- [ ] **Step 4: dispatch_event 修改 — 分发 LogWindow**

在 `dispatch_event` 的 `match self.focus` 中添加 `FocusArea::LogWindow` 分支：

```rust
FocusArea::LogWindow => {
    if let Some(ev) = self.log_window.handle_event(&event, true) {
        self.handle_app_event(ev).await;
    }
}
```

- [ ] **Step 5: 编译检查**

Run: `cargo check -p fi-code-core`
Expected: 编译通过

- [ ] **Step 6: Commit**

```bash
git add crates/tui/src/app.rs
git commit -m "feat(tui): hover focus switch and LogWindow focus support"
```

---

### Task 3: LeftDrawer 添加滚动条和滚轮支持

**Files:**
- Modify: `crates/tui/src/components/left_drawer.rs`

- [ ] **Step 1: 添加 scroll_offset 和导入**

修改 `LeftDrawer` 结构体：

```rust
use ratatui::widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState};

pub struct LeftDrawer {
    files: Vec<FileNode>,
    selected_index: usize,
    expanded_folders: std::collections::HashSet<String>,
    scroll_offset: usize,
}

impl LeftDrawer {
    pub fn new() -> Self {
        Self {
            files: Vec::new(),
            selected_index: 0,
            expanded_folders: std::collections::HashSet::new(),
            scroll_offset: 0,
        }
    }
}
```

- [ ] **Step 2: 添加 scroll 辅助方法**

```rust
impl LeftDrawer {
    pub fn scroll_up(&mut self, delta: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(delta);
    }

    pub fn scroll_down(&mut self, delta: usize) {
        let max = self.files.len().saturating_sub(1);
        self.scroll_offset = (self.scroll_offset + delta).min(max);
    }
}
```

- [ ] **Step 3: 修改 draw — skip 截断 + Scrollbar**

```rust
fn draw(&self, frame: &mut Frame, area: Rect, theme: &Theme, is_focused: bool) {
    let border_type = if is_focused {
        ratatui::widgets::BorderType::Double
    } else {
        ratatui::widgets::BorderType::Plain
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(border_type)
        .border_style(Style::default().fg(theme.border))
        .title("Files")
        .style(theme.drawer_style());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let viewport_height = inner.height as usize;

    let items: Vec<Line> = self
        .files
        .iter()
        .skip(self.scroll_offset)
        .take(viewport_height)
        .enumerate()
        .map(|(i, file)| {
            let actual_index = self.scroll_offset + i;
            let indent = "  ".repeat(file.depth);
            let icon = if file.is_dir { "📁 " } else { "📄 " };
            let style = if actual_index == self.selected_index {
                theme.style_selection()
            } else {
                theme.style_primary()
            };

            Line::from(vec![Span::styled(
                format!("{}{}{}", indent, icon, file.name),
                style,
            )])
        })
        .collect();

    let paragraph = Paragraph::new(items);
    frame.render_widget(paragraph, inner);

    // 渲染 scrollbar（内容超出时）
    if self.files.len() > viewport_height {
        let mut scrollbar_state = ScrollbarState::default()
            .content_length(self.files.len().saturating_sub(1))
            .position(self.scroll_offset)
            .viewport_content_length(viewport_height);

        let scrollbar = Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .style(Style::default().fg(theme.border));

        frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }
}
```

- [ ] **Step 4: 修改 handle_event — 添加滚轮支持**

```rust
fn handle_event(&mut self, event: &Event, _focus: bool) -> Option<AppEvent> {
    if let Event::Key(key) = event {
        if key.kind != KeyEventKind::Press {
            return None;
        }

        match key.code {
            KeyCode::Up => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                }
                None
            }
            KeyCode::Down => {
                if self.selected_index < self.files.len().saturating_sub(1) {
                    self.selected_index += 1;
                }
                None
            }
            KeyCode::Enter => {
                if let Some(file) = self.files.get(self.selected_index) {
                    return Some(AppEvent::SelectFile(file.path.clone()));
                }
                None
            }
            _ => None,
        }
    } else if let Event::Mouse(mouse) = event {
        use crossterm::event::MouseEventKind;
        match mouse.kind {
            MouseEventKind::ScrollUp => {
                self.scroll_up(3);
                None
            }
            MouseEventKind::ScrollDown => {
                self.scroll_down(3);
                None
            }
            _ => None,
        }
    } else {
        None
    }
}
```

- [ ] **Step 5: 更新单元测试**

在 `tests` 模块中添加滚动边界测试：

```rust
#[test]
fn test_scroll_boundary() {
    let mut drawer = LeftDrawer::new();
    drawer.set_files(vec![
        FileNode { path: "a".into(), name: "a".into(), is_dir: false, depth: 0 },
        FileNode { path: "b".into(), name: "b".into(), is_dir: false, depth: 0 },
        FileNode { path: "c".into(), name: "c".into(), is_dir: false, depth: 0 },
    ]);

    drawer.scroll_down(10);
    assert_eq!(drawer.scroll_offset, 2);

    drawer.scroll_up(1);
    assert_eq!(drawer.scroll_offset, 1);

    drawer.scroll_up(10);
    assert_eq!(drawer.scroll_offset, 0);
}
```

- [ ] **Step 6: 编译 + 测试**

Run: `cargo test -p fi-code-core left_drawer::tests`
Expected: 全部通过

- [ ] **Step 7: Commit**

```bash
git add crates/tui/src/components/left_drawer.rs
git commit -m "feat(tui): add scrollbar and mouse wheel scroll to LeftDrawer"
```

---

### Task 4: RightDrawer 添加滚动条和滚轮支持

**Files:**
- Modify: `crates/tui/src/components/right_drawer.rs`

- [ ] **Step 1: 添加 scroll_offset 和导入**

```rust
use ratatui::widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState};

pub struct RightDrawer {
    sessions: Vec<SessionMeta>,
    selected_index: usize,
    filter: String,
    filter_active: bool,
    scroll_offset: usize,
}

impl RightDrawer {
    pub fn new() -> Self {
        Self {
            sessions: Vec::new(),
            selected_index: 0,
            filter: String::new(),
            filter_active: false,
            scroll_offset: 0,
        }
    }
}
```

- [ ] **Step 2: 添加 scroll 辅助方法**

```rust
impl RightDrawer {
    pub fn scroll_up(&mut self, delta: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(delta);
    }

    pub fn scroll_down(&mut self, delta: usize) {
        let max = self.sessions.len().saturating_sub(1);
        self.scroll_offset = (self.scroll_offset + delta).min(max);
    }
}
```

- [ ] **Step 3: 修改 draw — 支持滚动和 scrollbar**

将整个 inner 区域作为滚动区域：

```rust
fn draw(&self, frame: &mut Frame, area: Rect, theme: &Theme, is_focused: bool) {
    let border_type = if is_focused {
        ratatui::widgets::BorderType::Double
    } else {
        ratatui::widgets::BorderType::Plain
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(border_type)
        .border_style(Style::default().fg(theme.border))
        .title("Tasks & Changes")
        .style(theme.drawer_style());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let viewport_height = inner.height as usize;

    let mut all_lines = vec![
        Line::styled("📋 Tasks", theme.style_primary().add_modifier(Modifier::BOLD)),
        Line::styled("  No active tasks", theme.style_muted()),
        Line::styled("", theme.style_primary()),
        Line::styled("📁 Changes", theme.style_primary().add_modifier(Modifier::BOLD)),
        Line::styled("  No changes yet", theme.style_muted()),
    ];

    for (i, session) in self.sessions.iter().enumerate() {
        if i == 0 {
            all_lines.push(Line::styled("", theme.style_primary()));
            all_lines.push(Line::styled("📝 Sessions", theme.style_primary().add_modifier(Modifier::BOLD)));
        }
        let marker = if session.is_current { "● " } else { "○ " };
        let style = if i == self.selected_index {
            theme.style_selection()
        } else {
            theme.style_primary()
        };
        all_lines.push(Line::styled(
            format!("  {}{} ({})", marker, session.name, session.message_count),
            style,
        ));
    }

    let visible_lines: Vec<Line> = all_lines
        .into_iter()
        .skip(self.scroll_offset)
        .take(viewport_height)
        .collect();

    frame.render_widget(Paragraph::new(visible_lines), inner);

    let total_lines = self.sessions.len() + 5;
    if total_lines > viewport_height {
        let mut scrollbar_state = ScrollbarState::default()
            .content_length(total_lines.saturating_sub(1))
            .position(self.scroll_offset)
            .viewport_content_length(viewport_height);

        let scrollbar = Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .style(Style::default().fg(theme.border));

        frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }
}
```

- [ ] **Step 4: 修改 handle_event — 添加滚轮支持**

```rust
fn handle_event(&mut self, event: &Event, _focus: bool) -> Option<AppEvent> {
    if let Event::Mouse(mouse) = event {
        use crossterm::event::MouseEventKind;
        match mouse.kind {
            MouseEventKind::ScrollUp => {
                self.scroll_up(3);
                None
            }
            MouseEventKind::ScrollDown => {
                self.scroll_down(3);
                None
            }
            _ => None,
        }
    } else {
        None
    }
}
```

- [ ] **Step 5: 编译 + 测试**

Run: `cargo test -p fi-code-core right_drawer::tests`
Expected: 全部通过

- [ ] **Step 6: Commit**

```bash
git add crates/tui/src/components/right_drawer.rs
git commit -m "feat(tui): add scrollbar and mouse wheel scroll to RightDrawer"
```

---

### Task 5: LogWindow 添加滚动条和滚轮支持

**Files:**
- Modify: `crates/tui/src/components/log_window.rs`

- [ ] **Step 1: draw 方法添加 Scrollbar**

在 `draw` 方法 paragraph 渲染之后添加：

```rust
let total_lines = text_lines.len();
if total_lines > visible_height && visible_height > 0 {
    let content_len = self.lines.len().saturating_sub(1);
    let max_start = content_len.saturating_sub(visible_height);
    let position = max_start.saturating_sub(self.scroll_offset);

    let mut scrollbar_state = ScrollbarState::default()
        .content_length(content_len)
        .position(position)
        .viewport_content_length(visible_height);

    let scrollbar = Scrollbar::default()
        .orientation(ScrollbarOrientation::VerticalRight)
        .style(Style::default().fg(theme.border));

    frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
}
```

- [ ] **Step 2: handle_event 添加滚轮支持**

修改 handle_event 以支持 Event::Mouse：

```rust
fn handle_event(&mut self, event: &Event, _focus: bool) -> Option<crate::tui::event::AppEvent> {
    if !self.visible {
        return None;
    }
    if let Event::Key(key) = event {
        if key.kind != KeyEventKind::Press {
            return None;
        }
        match key.code {
            KeyCode::Up => {
                self.scroll_up(1);
                return None;
            }
            KeyCode::Down => {
                self.scroll_down(1);
                return None;
            }
            KeyCode::PageUp => {
                self.scroll_up(10);
                return None;
            }
            KeyCode::PageDown => {
                self.scroll_down(10);
                return None;
            }
            _ => {}
        }
    } else if let Event::Mouse(mouse) = event {
        use crossterm::event::MouseEventKind;
        match mouse.kind {
            MouseEventKind::ScrollUp => {
                self.scroll_up(3);
            }
            MouseEventKind::ScrollDown => {
                self.scroll_down(3);
            }
            _ => {}
        }
    }
    None
}
```

- [ ] **Step 3: 编译 + 测试**

Run: `cargo test -p fi-code-core log_window::tests`
Expected: 全部通过

- [ ] **Step 4: Commit**

```bash
git add crates/tui/src/components/log_window.rs
git commit -m "feat(tui): add scrollbar and mouse wheel scroll to LogWindow"
```

---

### Task 6: 集成测试与最终验证

- [ ] **Step 1: 运行全部单元测试**

Run: `cargo test -p fi-code-core`
Expected: 全部通过（除了已有的 status_bar 测试失败，与本次改动无关）

- [ ] **Step 2: 运行 BDD 测试**

Run: `cargo test --test bdd`
Expected: 22/22 通过

- [ ] **Step 3: 运行 E2E 测试**

Run: `cargo test --test e2e_cli --test e2e_tui --test tui_flow_e2e`
Expected: 全部通过

- [ ] **Step 4: Clippy 检查**

Run: `cargo clippy -p fi-code-core -- -D warnings`
Expected: 通过

- [ ] **Step 5: Commit**

```bash
git commit --allow-empty -m "feat(tui): complete hover focus + scrollbar implementation"
```

---

## Self-Review Checklist

**1. Spec coverage:**
- [x] 全局 hover 焦点切换 -> Task 2 Step 2
- [x] FocusArea 添加 LogWindow -> Task 1
- [x] LeftDrawer 滚动条 + 滚轮 -> Task 3
- [x] RightDrawer 滚动条 + 滚轮 -> Task 4
- [x] LogWindow 滚动条 + 滚轮 -> Task 5
- [x] 键盘焦点循环包含 LogWindow -> Task 2 Step 3
- [x] 滚动条样式跟随主题 -> Task 3-5 draw 中的 theme.border

**2. Placeholder scan:**
- [x] 无 TBD/TODO
- [x] 所有步骤包含具体代码
- [x] 无 "类似 Task X" 的引用

**3. Type 一致性：**
- [x] scroll_offset: usize 在所有组件中一致
- [x] scroll_up/down(delta: usize) 签名一致
- [x] FocusArea::LogWindow 在 event.rs、app.rs 中一致
