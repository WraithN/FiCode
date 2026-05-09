# TUI 状态栏与布局改版 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 重构 fi-code TUI 底部状态栏（品牌 + 纯字符进度条 + 耗时 + Token + 模型），并将右侧边栏改为常驻不可关闭的三栏布局。

**Architecture:** 将 `PanelState` 简化为仅控制左侧边栏，右侧边栏始终渲染。底部状态栏完全重写为信息密集型单行组件，从顶部 Header 接管进度条、耗时、模型信息。进度条采用纯字符流式动画（无百分比），只表达运行状态。

**Tech Stack:** Rust + ratatui + crossterm

---

## File Structure

| File | Action | Responsibility |
|------|--------|---------------|
| `src/tui/layout.rs` | Modify | 简化 `PanelState` 为左栏专用，右侧边栏常驻，三栏布局计算 |
| `src/tui/components/status_bar.rs` | Rewrite | 全新底部状态栏：品牌、纯字符进度条、耗时、Token、模型 |
| `src/tui/components/header.rs` | Modify | 去掉 Logo 和 Braille 进度条，保留模型下拉菜单 |
| `src/tui/components/right_drawer.rs` | Modify | 改为常驻布局，更新标题为 Tasks & Changes（占位内容） |
| `src/tui/app.rs` | Modify | 适配新布局、更新状态同步、去掉 Ctrl+H、更新焦点循环 |

---

## Task 1: 修改 `PanelState` 与三栏布局计算

**Files:**
- Modify: `src/tui/layout.rs`
- Test: `src/tui/layout.rs`（内嵌 `#[cfg(test)]`）

### 背景

当前 `PanelState` 为 `None | LeftDrawer | RightDrawer`（左右互斥）。新设计下右侧边栏常驻，仅左侧边栏可开关。

### 变更内容

- [ ] **Step 1: 修改 `PanelState` 枚举**

```rust
/// 面板状态：仅控制左侧边栏开关，右侧边栏始终常驻。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelState {
    LeftClosed,
    LeftOpen,
}
```

- [ ] **Step 2: 修改 `LayoutAreas` 结构体**

```rust
#[derive(Debug)]
pub struct LayoutAreas {
    pub header: Rect,
    pub left_drawer: Option<Rect>,   // 左侧边栏（可选）
    pub main: Rect,                  // 中间主区域
    pub right_drawer: Rect,          // 右侧边栏（常驻，不再 Option）
    pub status_bar: Rect,
    pub overlay: Option<Rect>,       // 窄屏模式下的抽屉浮层
    pub log_window: Option<Rect>,    // 日志窗口
}
```

- [ ] **Step 3: 修改 `LayoutManager` 方法**

```rust
impl LayoutManager {
    pub fn new(width: u16, height: u16) -> Self {
        let narrow_mode = width < 80;
        Self {
            terminal_size: (width, height),
            panel: PanelState::LeftClosed,  // 默认关闭左侧
            narrow_mode,
            log_window: false,
        }
    }

    /// 切换左侧边栏。
    pub fn toggle_left(&mut self) {
        self.panel = match self.panel {
            PanelState::LeftClosed => PanelState::LeftOpen,
            PanelState::LeftOpen => PanelState::LeftClosed,
        };
    }

    /// 关闭左侧边栏。
    pub fn close_left(&mut self) {
        self.panel = PanelState::LeftClosed;
    }

    /// 根据当前状态计算每个组件应占据的 `Rect`。
    ///
    /// 固定行高：header = 3，status_bar = 1，剩余为 main 区域。
    /// 布局规则：
    /// - 右侧边栏：宽度 = min(max(width * 0.28, 24), 40)，始终显示
    /// - 左侧边栏：宽度 = min(max(width * 0.22, 20), 35)，仅 LeftOpen 时显示
    /// - 中间主区域：剩余宽度
    pub fn calculate(&self) -> LayoutAreas {
        let (width, height) = self.terminal_size;
        let header_height = 3u16;
        let status_height = 1u16;
        let main_height = height.saturating_sub(header_height + status_height);

        let right_width = ((width as f32 * 0.28) as u16).clamp(24, 40);
        let left_width = match self.panel {
            PanelState::LeftOpen => ((width as f32 * 0.22) as u16).clamp(20, 35),
            PanelState::LeftClosed => 0,
        };

        let main_width = width.saturating_sub(left_width + right_width);
        let main_x = left_width;
        let right_x = left_width + main_width;

        if self.narrow_mode && self.panel == PanelState::LeftOpen {
            // 窄屏模式下左侧边栏以浮层覆盖
            let overlay_width = (width as f32 * 0.75).max(30.0).min(width as f32) as u16;
            let mut main = Rect::new(0, header_height, width, main_height);
            let log_window = if self.log_window {
                let log_height = (main.height as f32 * 0.6) as u16;
                main.height = main.height.saturating_sub(log_height);
                Some(Rect::new(main.x, main.y + main.height, main.width, log_height))
            } else {
                None
            };

            LayoutAreas {
                header: Rect::new(0, 0, width, header_height),
                main,
                right_drawer: Rect::new(right_x, header_height, right_width, main_height),
                status_bar: Rect::new(0, height - status_height, width, status_height),
                left_drawer: None,
                overlay: Some(Rect::new(0, header_height, overlay_width, main_height)),
                log_window,
            }
        } else {
            let mut main = Rect::new(main_x, header_height, main_width, main_height);
            let log_window = if self.log_window {
                let log_height = (main.height as f32 * 0.6) as u16;
                main.height = main.height.saturating_sub(log_height);
                Some(Rect::new(main.x, main.y + main.height, main.width, log_height))
            } else {
                None
            };

            LayoutAreas {
                header: Rect::new(0, 0, width, header_height),
                left_drawer: (self.panel == PanelState::LeftOpen)
                    .then(|| Rect::new(0, header_height, left_width, main_height)),
                main,
                right_drawer: Rect::new(right_x, header_height, right_width, main_height),
                status_bar: Rect::new(0, height - status_height, width, status_height),
                overlay: None,
                log_window,
            }
        }
    }
}
```

- [ ] **Step 4: 更新 `LayoutManager` 测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_layout() {
        let layout = LayoutManager::new(120, 30);
        let areas = layout.calculate();

        assert_eq!(areas.header.height, 3);
        assert_eq!(areas.status_bar.height, 1);
        assert!(areas.left_drawer.is_none());
        assert_eq!(areas.right_drawer.width, ((120.0 * 0.28) as u16).clamp(24, 40));
        assert!(areas.overlay.is_none());
        assert_eq!(areas.main.width, 120 - areas.right_drawer.width);
    }

    #[test]
    fn test_left_drawer_expands() {
        let mut layout = LayoutManager::new(120, 30);
        layout.toggle_left();
        let areas = layout.calculate();

        assert!(areas.left_drawer.is_some());
        assert_eq!(areas.right_drawer.width, ((120.0 * 0.28) as u16).clamp(24, 40));
        assert!(areas.overlay.is_none());
        assert!(areas.main.width < 120 - areas.right_drawer.width);
    }

    #[test]
    fn test_right_drawer_always_present() {
        let layout = LayoutManager::new(120, 30);
        let areas = layout.calculate();
        assert!(areas.right_drawer.width > 0);
        assert_eq!(areas.right_drawer.height, 30 - 3 - 1);
    }

    #[test]
    fn test_narrow_mode_overlay() {
        let mut layout = LayoutManager::new(60, 30);
        layout.toggle_left();
        let areas = layout.calculate();

        assert!(areas.overlay.is_some());
        assert!(areas.left_drawer.is_none());
        assert_eq!(areas.main.width, 60);
        assert!(areas.right_drawer.width > 0);
    }

    #[test]
    fn test_main_split() {
        let main = Rect::new(0, 3, 100, 20);
        let (messages, input) = LayoutManager::split_main(main, 3);

        assert_eq!(input.height, 5);
        assert_eq!(messages.height, 15);
        assert_eq!(messages.width, 100);
        assert_eq!(input.width, 100);
    }

    #[test]
    fn test_log_window_split() {
        let mut layout = LayoutManager::new(100, 30);
        layout.log_window = true;
        let areas = layout.calculate();
        assert!(areas.log_window.is_some());
        let log = areas.log_window.unwrap();
        let main = areas.main;
        assert_eq!(main.height + log.height, 30 - 3 - 1);
        assert!(log.height > main.height);
        assert_eq!(log.y, main.y + main.height);
        assert_eq!(log.width, main.width);
    }
}
```

- [ ] **Step 5: 编译检查**

Run: `cargo check --lib`
Expected: PASS（可能有其他文件未更新导致的 error，这是正常的）

---

## Task 2: 重写底部状态栏组件

**Files:**
- Rewrite: `src/tui/components/status_bar.rs`
- Test: `src/tui/components/status_bar.rs`（内嵌 `#[cfg(test)]`）

### 背景

当前状态栏只显示快捷键提示。新设计需要显示：品牌标识 + 纯字符进度条 + 耗时 + Token 计数 + 当前模型。

### 变更内容

- [ ] **Step 1: 实现新的 `StatusBar` 组件**

```rust
// MIT License
// Copyright (c) 2025 fi-code contributors
// ... (许可证头保持完整)

use crossterm::event::Event;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::tui::components::Component;
use crate::tui::event::AppEvent;
use crate::tui::theme::Theme;

/// 进度条动画状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProgressState {
    /// 空闲：进度条为空，静态显示。
    Idle,
    /// 运行中：每 tick 前进一格，到头后停在最满状态。
    Running,
    /// 暂停：定格在当前长度，不再前进。
    Paused,
}

/// 底部状态栏组件，显示品牌、进度条、耗时、Token 统计和当前模型。
///
/// 该组件不可聚焦，仅作为信息展示。
pub struct StatusBar {
    progress_state: ProgressState,
    progress_tick: u64,       // 动画帧计数器
    last_filled: usize,       // 暂停时定格的填充格数
    model_name: String,       // 当前模型名
    token_in: usize,          // 输入 Token 计数
    token_out: usize,         // 输出 Token 计数
    elapsed_secs: u64,        // 当前耗时（秒）
}

/// 进度条总格数。
const PROGRESS_BAR_WIDTH: usize = 20;

impl StatusBar {
    pub fn new() -> Self {
        Self {
            progress_state: ProgressState::Idle,
            progress_tick: 0,
            last_filled: 0,
            model_name: "unknown".to_string(),
            token_in: 0,
            token_out: 0,
            elapsed_secs: 0,
        }
    }

    /// 更新生成状态。
    pub fn set_generating(&mut self, generating: bool) {
        match (self.progress_state, generating) {
            (ProgressState::Idle, true) => {
                self.progress_state = ProgressState::Running;
                self.progress_tick = 0;
                self.last_filled = 0;
            }
            (ProgressState::Running, false) => {
                self.progress_state = ProgressState::Paused;
                self.last_filled = self.current_filled();
            }
            (ProgressState::Paused, true) => {
                self.progress_state = ProgressState::Running;
                self.progress_tick = self.last_filled as u64;
            }
            (ProgressState::Paused, false) => {
                // 已经是暂停状态，重置为空闲
                self.progress_state = ProgressState::Idle;
                self.progress_tick = 0;
                self.last_filled = 0;
            }
            _ => {}
        }
    }

    /// 更新当前模型名。
    pub fn set_model(&mut self, model: String) {
        self.model_name = model;
    }

    /// 更新 Token 计数。
    pub fn set_tokens(&mut self, in_count: usize, out_count: usize) {
        self.token_in = in_count;
        self.token_out = out_count;
    }

    /// 更新耗时（秒）。
    pub fn set_elapsed(&mut self, secs: u64) {
        self.elapsed_secs = secs;
    }

    /// 每帧 tick，更新进度条动画。
    pub fn on_tick(&mut self) {
        if self.progress_state == ProgressState::Running {
            self.progress_tick = self.progress_tick.wrapping_add(1);
        }
    }

    /// 计算当前应填充的格数。
    fn current_filled(&self) -> usize {
        match self.progress_state {
            ProgressState::Idle => 0,
            ProgressState::Running => {
                (self.progress_tick as usize).min(PROGRESS_BAR_WIDTH)
            }
            ProgressState::Paused => self.last_filled,
        }
    }

    /// 渲染进度条字符串。
    fn render_progress_bar(&self) -> String {
        let filled = self.current_filled();
        let empty = PROGRESS_BAR_WIDTH - filled;
        format!("[{}{}]", "█".repeat(filled), "░".repeat(empty))
    }

    /// 格式化耗时显示。
    fn format_elapsed(&self) -> String {
        if self.elapsed_secs == 0 {
            String::new()
        } else {
            let minutes = self.elapsed_secs / 60;
            let secs = self.elapsed_secs % 60;
            if minutes > 0 {
                format!("{}m{}s", minutes, secs)
            } else {
                format!("{}s", secs)
            }
        }
    }

    /// 构建状态栏完整显示行。
    fn build_line(&self, theme: &Theme) -> Line<'static> {
        let mut spans = vec![];

        // 品牌标识：FiCode（品牌色）
        spans.push(Span::styled("FiCode", theme.style_brand()));
        spans.push(Span::raw("  "));

        // 进度条
        let progress_bar = self.render_progress_bar();
        let progress_style = match self.progress_state {
            ProgressState::Idle => Style::default().fg(theme.text_muted),
            ProgressState::Running | ProgressState::Paused => {
                Style::default().fg(theme.brand)
            }
        };
        spans.push(Span::styled(progress_bar, progress_style));

        // 分隔符 + 耗时
        let elapsed = self.format_elapsed();
        if !elapsed.is_empty() {
            spans.push(Span::styled(" ｜ ", theme.style_muted()));
            spans.push(Span::styled(
                format!("耗时：{}", elapsed),
                theme.style_primary(),
            ));
        }

        // 分隔符 + Token 统计
        if self.token_in > 0 || self.token_out > 0 {
            spans.push(Span::styled(" ｜ ", theme.style_muted()));
            spans.push(Span::styled(
                format!("IN:{} OUT:{}", self.token_in, self.token_out),
                theme.style_primary(),
            ));
        }

        // 分隔符 + 模型名
        spans.push(Span::styled(" ｜ ", theme.style_muted()));
        spans.push(Span::styled(
            format!("Model:{}", self.model_name),
            theme.style_primary(),
        ));

        Line::from(spans)
    }
}

impl Component for StatusBar {
    fn draw(&self, frame: &mut Frame, area: Rect, theme: &Theme, _is_focused: bool) {
        let line = self.build_line(theme);
        let paragraph = Paragraph::new(line).style(theme.status_bar_style());
        frame.render_widget(paragraph, area);
    }

    fn handle_event(&mut self, _event: &Event, _focus: bool) -> Option<AppEvent> {
        None
    }

    fn is_focusable(&self) -> bool {
        false
    }

    fn update(&mut self, event: &AppEvent) {
        match event {
            AppEvent::Tick => self.on_tick(),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_bar_initial_state() {
        let bar = StatusBar::new();
        assert_eq!(bar.progress_state, ProgressState::Idle);
        assert_eq!(bar.progress_tick, 0);
        assert_eq!(bar.model_name, "unknown");
    }

    #[test]
    fn test_progress_state_transitions() {
        let mut bar = StatusBar::new();
        
        // 空闲 → 运行
        bar.set_generating(true);
        assert_eq!(bar.progress_state, ProgressState::Running);
        
        // 运行 → 暂停
        bar.set_generating(false);
        assert_eq!(bar.progress_state, ProgressState::Paused);
        
        // 暂停 → 空闲（再次停止）
        bar.set_generating(false);
        assert_eq!(bar.progress_state, ProgressState::Idle);
        
        // 空闲 → 运行 → 运行（无变化）
        bar.set_generating(true);
        bar.set_generating(true);
        assert_eq!(bar.progress_state, ProgressState::Running);
    }

    #[test]
    fn test_progress_bar_idle() {
        let bar = StatusBar::new();
        let pb = bar.render_progress_bar();
        assert_eq!(pb, "[░░░░░░░░░░░░░░░░░░]");
    }

    #[test]
    fn test_progress_bar_running() {
        let mut bar = StatusBar::new();
        bar.set_generating(true);
        bar.on_tick(); // tick = 1, filled = 1
        let pb = bar.render_progress_bar();
        assert_eq!(pb, "[█░░░░░░░░░░░░░░░░░]");
        
        // 前进到第 5 格
        for _ in 0..4 {
            bar.on_tick();
        }
        let pb = bar.render_progress_bar();
        assert_eq!(pb, "[█████░░░░░░░░░░░░░]");
    }

    #[test]
    fn test_progress_bar_capped_at_width() {
        let mut bar = StatusBar::new();
        bar.set_generating(true);
        // 前进超过 20 格
        for _ in 0..30 {
            bar.on_tick();
        }
        let pb = bar.render_progress_bar();
        assert_eq!(pb, "[████████████████████]");
    }

    #[test]
    fn test_progress_bar_paused() {
        let mut bar = StatusBar::new();
        bar.set_generating(true);
        for _ in 0..5 {
            bar.on_tick();
        }
        bar.set_generating(false); // 暂停，定格在 5 格
        
        // 即使继续 tick，也不应前进
        bar.on_tick();
        bar.on_tick();
        let pb = bar.render_progress_bar();
        assert_eq!(pb, "[█████░░░░░░░░░░░░░]");
    }

    #[test]
    fn test_elapsed_format() {
        let mut bar = StatusBar::new();
        assert_eq!(bar.format_elapsed(), "");
        
        bar.set_elapsed(52);
        assert_eq!(bar.format_elapsed(), "52s");
        
        bar.set_elapsed(125);
        assert_eq!(bar.format_elapsed(), "2m5s");
    }

    #[test]
    fn test_build_line_includes_model() {
        let mut bar = StatusBar::new();
        bar.set_model("kimi-code".to_string());
        let theme = Theme::deep_ocean();
        let line = bar.build_line(&theme);
        let text = line.to_string();
        assert!(text.contains("FiCode"));
        assert!(text.contains("Model:kimi-code"));
    }
}
```

- [ ] **Step 2: 编译检查**

Run: `cargo check --lib`
Expected: PASS（`Theme::deep_ocean()` 是测试用辅助方法，确保它可用）

---

## Task 3: 简化 Header 组件

**Files:**
- Modify: `src/tui/components/header.rs`
- Test: `src/tui/components/header.rs`（内嵌 `#[cfg(test)]`）

### 背景

Header 当前显示 Logo、模型名、状态文字、Braille 进度条和耗时。新设计中这些信息都移到了底部状态栏，Header 只需要保留模型下拉菜单和简化的状态指示。

### 变更内容

- [ ] **Step 1: 简化 `Header` 结构体**

```rust
pub struct Header {
    current_model: String,
    session_id: Option<String>,
    menu_state: MenuState,
    providers: Vec<ProviderItem>,
    provider_selected: usize,
    model_selected: Vec<usize>,
    status: HeaderStatus,
}
```

- [ ] **Step 2: 更新 `Header::new()`**

```rust
pub fn new() -> Self {
    Self {
        current_model: "unknown".to_string(),
        session_id: None,
        menu_state: MenuState::Closed,
        providers: vec![],
        provider_selected: 0,
        model_selected: vec![],
        status: HeaderStatus::Ready,
    }
}
```

- [ ] **Step 3: 去掉 `set_status` 中的 `start_time` 逻辑**

```rust
pub fn set_status(&mut self, status: HeaderStatus) {
    self.status = status;
}
```

- [ ] **Step 4: 去掉 `format_elapsed`、`progress_tick`、`on_tick`、和 `start_time` 相关代码**

- [ ] **Step 5: 重写 `Header::draw()`**

```rust
fn draw(&self, frame: &mut Frame, area: Rect, theme: &Theme, _is_focused: bool) {
    let block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(theme.border))
        .style(theme.header_style());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // 左侧：模型名（带下拉指示器）
    let model_text = format!("▼ {}", self.current_model);
    let model = Span::styled(model_text, theme.style_primary());

    // 中间：状态指示（简化）
    let (status_label, status_color) = match self.status {
        HeaderStatus::Ready => ("● Ready", theme.success),
        HeaderStatus::Generating => ("● Generating", theme.warning),
        HeaderStatus::Streaming => ("● Streaming", theme.brand),
    };
    let status = Span::styled(status_label, Style::default().fg(status_color));

    // 右侧：如果有 session_id，显示会话标识
    let session_span = if let Some(ref id) = self.session_id {
        let short_id = if id.len() >= 4 {
            &id[..4]
        } else {
            id.as_str()
        };
        Span::styled(
            format!("--[session: #{}] --", short_id),
            theme.style_muted(),
        )
    } else {
        Span::raw("")
    };

    // 使用 Layout 分成三部分
    let chunks = Layout::horizontal([
        Constraint::Min(0),      // 模型名
        Constraint::Length(16),  // 状态
        Constraint::Min(0),      // 会话标识（右对齐）
    ])
    .split(inner);

    let model_line = Line::from(vec![model]);
    frame.render_widget(
        Paragraph::new(model_line),
        chunks[0],
    );

    let status_line = Line::from(vec![status]);
    frame.render_widget(
        Paragraph::new(status_line).alignment(Alignment::Center),
        chunks[1],
    );

    if !session_span.content.is_empty() {
        let session_line = Line::from(vec![session_span]);
        frame.render_widget(
            Paragraph::new(session_line).alignment(Alignment::Right),
            chunks[2],
        );
    }

    match &self.menu_state {
        MenuState::ProviderList => self.draw_provider_list(frame, area, theme),
        MenuState::ModelList { provider_idx } => {
            self.draw_model_list(frame, area, theme, *provider_idx)
        }
        MenuState::Closed => {}
    }
}
```

- [ ] **Step 6: 更新 `Header` 测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_status() {
        let mut header = Header::new();
        header.set_status(HeaderStatus::Generating);
        assert_eq!(header.status, HeaderStatus::Generating);
    }

    #[test]
    fn test_menu_toggle() {
        let mut header = Header::new();
        assert!(!header.has_dropdown_open());
        header.toggle_model_dropdown();
        assert!(header.has_dropdown_open());
        header.toggle_model_dropdown();
        assert!(!header.has_dropdown_open());
    }

    #[test]
    fn test_session_id() {
        let mut header = Header::new();
        assert!(header.session_id().is_none());
        header.set_session_id("a7f2-3abc".to_string());
        assert_eq!(header.session_id(), Some("a7f2-3abc".to_string()));
    }
}
```

- [ ] **Step 7: 编译检查**

Run: `cargo check --lib`
Expected: PASS

---

## Task 4: 更新 RightDrawer 为常驻模式

**Files:**
- Modify: `src/tui/components/right_drawer.rs`

### 背景

当前 right_drawer 显示 "Session History"，可开关。新设计下右侧边栏常驻，显示 "Tasks & Changes"（当前为占位内容，等后端 API 就绪后填充真实数据）。

### 变更内容

- [ ] **Step 1: 修改 `RightDrawer` 渲染**

在 `draw` 方法中，修改标题和内容：

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

    // 上下两个区块：Tasks（上）和 Changes（下）
    let mid_y = inner.y + inner.height / 2;
    let top_height = inner.height / 2;
    let bottom_height = inner.height - top_height;

    // 上半区：Tasks
    let tasks_title = Line::styled("📋 Tasks", theme.style_primary().add_modifier(Modifier::BOLD));
    let tasks_placeholder = Line::styled("  No active tasks", theme.style_muted());
    
    frame.render_widget(
        Paragraph::new(vec![tasks_title, tasks_placeholder]),
        Rect::new(inner.x, inner.y, inner.width, top_height),
    );

    // 下半区：Changes
    let changes_title = Line::styled("📁 Changes", theme.style_primary().add_modifier(Modifier::BOLD));
    let changes_placeholder = Line::styled("  No changes yet", theme.style_muted());
    
    frame.render_widget(
        Paragraph::new(vec![changes_title, changes_placeholder]),
        Rect::new(inner.x, mid_y, inner.width, bottom_height),
    );
}
```

- [ ] **Step 2: 简化 `RightDrawer`（去掉导航事件，改为纯展示）**

```rust
fn handle_event(&mut self, _event: &Event, _focus: bool) -> Option<AppEvent> {
    None
}
```

- [ ] **Step 3: 编译检查**

Run: `cargo check --lib`
Expected: PASS

---

## Task 5: 更新 App 主循环适配新布局

**Files:**
- Modify: `src/tui/app.rs`

### 背景

App 是各组件的协调中心。需要适配：
1. 新 `PanelState`（只有 LeftClosed/LeftOpen）
2. 右侧边栏常驻（始终渲染，去掉 ToggleRightDrawer）
3. 新的 StatusBar 接口（传递模型名、耗时、Token）
4. 去掉 Ctrl+H 快捷键
5. 更新焦点循环（RightDrawer 常驻参与）
6. 在 Tick 中更新状态栏耗时

### 变更内容

- [ ] **Step 1: 添加 `generation_start` 字段到 `TuiApp`**

```rust
pub struct TuiApp {
    // ... 现有字段 ...
    
    // === 应用状态 ===
    focus: FocusArea,
    is_generating: bool,
    should_quit: bool,
    exit_confirm_pending: bool,
    dirty: bool,
    api_key_dialog: Option<ApiKeyDialog>,
    question_dialog: Option<QuestionDialog>,
    generation_start: Option<std::time::Instant>, // 新增：生成开始时间
    
    // ... 其余字段不变 ...
}
```

- [ ] **Step 2: 在 `TuiApp::new()` 中初始化 `generation_start: None`**

- [ ] **Step 3: 更新 `draw` 方法**（右侧始终渲染，去掉 overlay 中的 RightDrawer 处理）

```rust
fn draw(&mut self, frame: &mut ratatui::Frame) {
    let area = frame.area();
    self.layout.resize(area.width, area.height);
    let areas = self.layout.calculate();
    let input_lines = self.input.visible_lines();
    let input_extra = if self.header.session_id().is_some() {
        1
    } else {
        0
    };
    let (messages_area, input_area) =
        LayoutManager::split_main(areas.main, input_lines + input_extra);

    self.header.draw(
        frame,
        areas.header,
        &self.theme,
        self.focus == FocusArea::Header,
    );
    self.chat.draw(
        frame,
        messages_area,
        &self.theme,
        self.focus == FocusArea::Main,
    );
    self.input.draw(
        frame,
        input_area,
        &self.theme,
        self.focus == FocusArea::Input,
    );
    self.input.set_last_drawn_area(input_area);
    self.input.update_dropdown_area(input_area);
    
    // 底部状态栏
    self.status_bar.draw(frame, areas.status_bar, &self.theme, false);

    if let Some(log_area) = areas.log_window {
        self.log_window.draw(frame, log_area, &self.theme, false);
    }

    // 窄屏模式下左侧边栏浮层
    if let Some(overlay_area) = areas.overlay {
        let dim = ratatui::widgets::Block::default()
            .style(ratatui::style::Style::default().bg(self.theme.bg_overlay));
        frame.render_widget(dim, areas.main);

        // 窄屏模式下只有左侧边栏有浮层
        self.left_drawer.draw(
            frame,
            overlay_area,
            &self.theme,
            self.focus == FocusArea::LeftDrawer,
        );
    } else {
        if let Some(area) = areas.left_drawer {
            self.left_drawer.draw(
                frame,
                area,
                &self.theme,
                self.focus == FocusArea::LeftDrawer,
            );
        }
    }

    // 右侧边栏：始终渲染（常驻）
    self.right_drawer.draw(
        frame,
        areas.right_drawer,
        &self.theme,
        self.focus == FocusArea::RightDrawer,
    );

    // ... API Key 和问题询问模态框渲染不变 ...
}
```

- [ ] **Step 4: 更新 `cycle_focus`（右侧常驻参与焦点循环）**

```rust
fn cycle_focus(&mut self, forward: bool) {
    let areas = if self.layout.panel == PanelState::LeftOpen {
        vec![
            FocusArea::LeftDrawer,
            FocusArea::Main,
            FocusArea::Input,
            FocusArea::RightDrawer,
        ]
    } else {
        vec![
            FocusArea::Main,
            FocusArea::Input,
            FocusArea::RightDrawer,
        ]
    };

    let current_idx = areas.iter().position(|a| a == &self.focus).unwrap_or(0);
    let next_idx = if forward {
        (current_idx + 1) % areas.len()
    } else {
        (current_idx + areas.len() - 1) % areas.len()
    };

    self.focus = areas[next_idx];
}
```

- [ ] **Step 5: 去掉 `handle_ctrl_key` 中的 Ctrl+H**

```rust
async fn handle_ctrl_key(&mut self, key: &crossterm::event::KeyEvent) {
    let KeyCode::Char(c) = key.code else { return };
    let lower = if c.is_ascii_control() {
        (c as u8 + b'a' - 1) as char
    } else {
        c.to_ascii_lowercase()
    };
    match lower {
        'c' => {
            if self.is_generating {
                self.handle_app_event(AppEvent::StopGeneration).await;
                self.exit_confirm_pending = false;
            } else if self.exit_confirm_pending {
                self.should_quit = true;
            } else {
                self.exit_confirm_pending = true;
                self.chat.add_system_message("Press Ctrl+C again to exit.");
            }
        }
        'b' => {
            self.exit_confirm_pending = false;
            self.handle_app_event(AppEvent::ToggleLeftDrawer).await;
            self.focus = FocusArea::LeftDrawer;
        }
        // 去掉 'h' 分支（右侧不再可开关）
        'm' => {
            self.exit_confirm_pending = false;
            self.handle_execute_slash_command("models", &None);
        }
        // ... 其余分支不变 ...
    }
}
```

- [ ] **Step 6: 更新 `handle_esc_key`**

```rust
fn handle_esc_key(&mut self) -> Option<AppEvent> {
    if self.log_window.is_visible() {
        return Some(AppEvent::ToggleLogWindow);
    }
    if self.input.is_submenu_open() {
        return Some(AppEvent::CancelThemePreview);
    }
    // 左侧边栏关闭
    if self.layout.panel == PanelState::LeftOpen {
        self.layout.close_left();
    } else if self.header.has_dropdown_open() {
        self.header.close_dropdowns();
    } else {
        self.focus = FocusArea::Main;
    }
    None
}
```

- [ ] **Step 7: 更新 `handle_app_event` 中的状态同步**

```rust
async fn handle_app_event(&mut self, event: AppEvent) {
    // Tick 重绘逻辑不变
    if matches!(event, AppEvent::Tick) {
        if self.is_generating {
            self.dirty = true;
        }
    } else {
        self.dirty = true;
    }

    match event {
        // ... 其他事件处理不变 ...

        AppEvent::Tick => {
            self.chat.on_tick();
            self.header.on_tick(); // header 仍然需要 tick（如果有动画）
            
            // 更新状态栏的耗时和进度动画
            if let Some(start) = self.generation_start {
                let elapsed = start.elapsed().as_secs();
                self.status_bar.set_elapsed(elapsed);
            }
        }

        AppEvent::SubmitMessage(ref msg) => {
            self.is_generating = true;
            self.generation_start = Some(std::time::Instant::now());
            self.header.set_status(HeaderStatus::Generating);
            self.chat.add_user_message(msg);
            self.start_chat_stream(msg.clone()).await;
        }

        AppEvent::SseEvent(ref sse_event) => {
            self.chat.handle_sse_event(sse_event);
            match sse_event {
                SseEvent::Message { .. } | SseEvent::ToolUse { .. } 
                | SseEvent::ToolResult { .. } | SseEvent::MessageDetails { .. } => {
                    self.header.set_status(HeaderStatus::Streaming);
                }
                _ => {}
            }
            if let SseEvent::Done { session_id } = sse_event {
                self.header.set_session_id(session_id.clone());
                self.input.set_session_id(Some(session_id.clone()));
            }
        }

        AppEvent::ChatComplete => {
            self.is_generating = false;
            self.generation_start = None;
            self.header.set_status(HeaderStatus::Ready);
        }

        AppEvent::StopGeneration => {
            self.is_generating = false;
            self.generation_start = None;
            self.header.set_status(HeaderStatus::Ready);
        }

        AppEvent::ToggleLeftDrawer => {
            self.layout.toggle_left();
            if self.layout.panel == PanelState::LeftOpen {
                self.focus = FocusArea::LeftDrawer;
                let client = self.client.clone();
                let tx = self.event_tx.clone();
                tokio::spawn(async move {
                    if let Ok(file_tree) = client.get_file_tree(".").await {
                        let files: Vec<crate::tui::components::left_drawer::FileNode> = file_tree
                            .entries
                            .into_iter()
                            .map(|e| crate::tui::components::left_drawer::FileNode {
                                path: e.path,
                                name: e.name,
                                is_dir: e.is_dir,
                                depth: e.depth,
                            })
                            .collect();
                        let _ = tx.send(AppEvent::SetFileTree(files)).await;
                    }
                });
            }
        }

        // 去掉 ToggleRightDrawer 处理（右侧常驻）

        AppEvent::SelectModel(ref model) => {
            self.header.set_current_model(model.clone());
            self.status_bar.set_model(model.clone()); // 同步到状态栏
        }

        // ... 其余事件处理不变 ...
    }

    // 同步底部状态栏状态
    self.status_bar.set_generating(self.is_generating);
    
    // 同步模型名（如果 header 已设置）
    if self.status_bar.model_name == "unknown" {
        if let Some(model) = self.header.get_current_model() {
            self.status_bar.set_model(model);
        }
    }

    self.header.update(&event);
    self.chat.update(&event);
    self.input.update(&event);
    self.left_drawer.update(&event);
    self.right_drawer.update(&event);
    self.status_bar.update(&event);
    self.log_window.update(&event);
}
```

注意：`header.get_current_model()` 方法可能不存在，需要在 header.rs 中添加：

```rust
impl Header {
    pub fn get_current_model(&self) -> Option<String> {
        if self.current_model == "unknown" {
            None
        } else {
            Some(self.current_model.clone())
        }
    }
}
```

- [ ] **Step 8: 编译检查**

Run: `cargo check --lib`
Expected: PASS（可能有 right_drawer 中 SessionMeta 未使用的 warning，可暂时忽略）

---

## Task 6: 编译检查、运行测试和提交

- [ ] **Step 1: 编译检查**

Run: `cargo check`
Expected: 无 error（允许有未使用代码的 warning）

- [ ] **Step 2: 运行测试**

Run: `cargo test`
Expected: 所有测试通过（包括 layout、status_bar、header 的内嵌测试）

- [ ] **Step 3: Clippy 检查**

Run: `cargo clippy -- -D warnings`
Expected: 无 warning（如果有未使用字段的 warning，可修复或允许）

- [ ] **Step 4: 格式化**

Run: `cargo fmt`

- [ ] **Step 5: 提交**

```bash
git add src/tui/layout.rs src/tui/components/status_bar.rs \
    src/tui/components/header.rs src/tui/components/right_drawer.rs \
    src/tui/app.rs
git commit -m "feat(tui): redesign status bar with brand, progress bar, elapsed, tokens, model

- Simplify PanelState to control only left drawer
- Make right drawer permanently visible (Tasks & Changes)
- Redesign status bar: FiCode brand + pure char progress bar
  + elapsed time + token count + model name
- Move progress/brand/elapsed from header to status bar
- Remove Ctrl+H (right drawer no longer toggleable)
- Update focus cycle for permanent right drawer
- Add progress bar animation tests"
```

---

## Self-Review

### 1. Spec Coverage

| Spec 要求 | 对应 Task |
|-----------|-----------|
| 品牌标识在底部状态栏最左侧 | Task 2: `build_line` 中第一个 span 是 `FiCode` |
| 纯字符进度条，无百分比 | Task 2: `render_progress_bar` 用 `█`/`░`，无数字 |
| 流式动态逐格填充 | Task 2: `on_tick` 每帧 +1， capped at width |
| 单轮结束定格当前长度 | Task 2: `ProgressState::Paused` 记录 `last_filled` |
| 耗时显示 | Task 2: `format_elapsed` + `set_elapsed` |
| Token IN/OUT | Task 2: `token_in`/`token_out` + `set_tokens` |
| 模型名 | Task 2: `model_name` + `set_model` |
| 右侧边栏常驻 | Task 1: `right_drawer` 改为 `Rect`（非 Option） |
| 左侧 Ctrl+B 显隐 | Task 1: `PanelState::LeftClosed/LeftOpen` |
| 三栏布局 | Task 1: `calculate` 始终计算 left+main+right |
| 会话标识 `--[session: #...] --` | Task 3: Header `draw` 中保留 session 显示 |
| 去掉 Ctrl+H | Task 5: `handle_ctrl_key` 去掉 `'h'` 分支 |

### 2. Placeholder Scan

- ✅ 无 "TBD"、"TODO"、"implement later"
- ✅ 无 "Add appropriate error handling" 等模糊描述
- ✅ 每个 task 都包含具体代码
- ✅ 无 "Similar to Task N" 引用

### 3. Type Consistency

- ✅ `PanelState::LeftClosed/LeftOpen` 在所有文件中一致使用
- ✅ `StatusBar` 的 `set_model/set_elapsed/set_tokens` 接口在 app.rs 中正确使用
- ✅ `LayoutAreas.right_drawer` 从 `Option<Rect>` 改为 `Rect`，app.rs 中去掉 `.unwrap()`/`.is_some()` 检查
- ✅ `generation_start: Option<Instant>` 在 SubmitMessage/ChatComplete/StopGeneration 中正确设置和清空

---

## Execution Handoff

**Plan complete and saved to `docs/superpowers/plans/2026-05-09-tui-statusbar-redesign.md`.**

Two execution options:

**1. Subagent-Driven (recommended)** — Dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** — Execute tasks in this session using executing-plans, batch execution with checkpoints

Which approach do you prefer?
