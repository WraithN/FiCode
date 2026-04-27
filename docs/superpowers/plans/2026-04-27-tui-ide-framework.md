# TUI IDE 式框架重构实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 FiCode 从基础单栏 TUI 升级为 IDE 式框架，包含 1+2 抽屉布局、Token 化主题系统、Component 化架构、多行输入、以及后端 REST API 支持。

**Architecture:** 采用 Component Pattern 将界面拆分为 6 个独立组件（Header/LeftDrawer/Chat/Input/RightDrawer/StatusBar），通过统一的 `Component` trait 和 `AppEvent` 枚举通信。布局由 `LayoutManager` 统一计算，支持抽屉互斥和窄屏自适应。后端新增 `/api/sessions` 和 `/api/files` REST 端点，TUI 通过 `TuiClient` 统一获取数据。

**Tech Stack:** Rust, ratatui 0.28, crossterm 0.28, axum 0.7, serde, chrono

---

## 文件变更总览

### 新建文件（13 个）

| 文件 | 职责 |
|------|------|
| `src/tui/theme.rs` | Theme Token 系统 + 5 套预设 |
| `src/tui/layout.rs` | 布局计算、抽屉互斥、窄屏检测 |
| `src/tui/event.rs` | `AppEvent` / `FocusArea` / `SessionTemplate` 枚举 |
| `src/tui/components/mod.rs` | `Component` trait 定义 |
| `src/tui/components/header.rs` | Header：模型下拉、主题切换、新建会话 |
| `src/tui/components/left_drawer.rs` | 左侧文件导航抽屉 |
| `src/tui/components/right_drawer.rs` | 右侧会话历史抽屉 |
| `src/tui/components/chat.rs` | 主聊天区：消息列表、滚动、代码块 |
| `src/tui/components/input.rs` | 多行输入框、Slash 命令面板 |
| `src/tui/components/status_bar.rs` | 底部快捷键提示栏 |
| `src/server/session_api.rs` | 会话管理 REST API |
| `src/server/file_api.rs` | 文件树 + 内容 REST API |
| `src/server/models.rs` | API 请求/响应 DTO |

### 修改文件（6 个）

| 文件 | 变更内容 |
|------|----------|
| `src/tui/app.rs` | 全面重写：状态机、事件循环、组件集成 |
| `src/tui/mod.rs` | 导出新模块 |
| `src/tui/client.rs` | 扩展：新增 `list_sessions`, `create_session`, `switch_session`, `get_file_tree`, `get_file_content` 方法 |
| `src/server/mod.rs` | 导出新模块 |
| `src/server/server.rs` | 添加新路由 `/api/*` |
| `Cargo.toml` | 添加 `walkdir = "2.5"` 依赖（P2 文件树遍历） |

---

## Phase 1: 基础设施层

### Task 1: Theme Token 系统

**Files:**
- Create: `src/tui/theme.rs`
- Modify: `src/tui/mod.rs`

**Context:** 当前 TUI 颜色全部硬编码在 `ui.rs` 中（`Color::Cyan`, `Color::Green` 等）。需要替换为语义化的 Theme Token 系统。

- [ ] **Step 1: 创建 Theme 结构体和预设**

创建 `src/tui/theme.rs`：

```rust
use ratatui::style::{Color, Modifier, Style};

#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    pub bg_base: Color,
    pub bg_surface: Color,
    pub bg_overlay: Color,
    pub border: Color,
    pub text_primary: Color,
    pub text_secondary: Color,
    pub text_muted: Color,
    pub text_placeholder: Color,
    pub brand: Color,
    pub user: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    pub selection_bg: Color,
    pub selection_fg: Color,
    pub accent_hover: Color,
}

impl Theme {
    pub fn deep_ocean() -> Self {
        Self {
            bg_base: Color::from_u32(0x0d1117),
            bg_surface: Color::from_u32(0x161b22),
            bg_overlay: Color::from_u32(0x1a2332),
            border: Color::from_u32(0x30363d),
            text_primary: Color::from_u32(0xc9d1d9),
            text_secondary: Color::from_u32(0x8b949e),
            text_muted: Color::from_u32(0x484f58),
            text_placeholder: Color::from_u32(0x6e7681),
            brand: Color::from_u32(0x39d0d8),
            user: Color::from_u32(0xf0883e),
            success: Color::from_u32(0x3fb950),
            warning: Color::from_u32(0xd29922),
            error: Color::from_u32(0xf85149),
            selection_bg: Color::from_u32(0x264f78),
            selection_fg: Color::White,
            accent_hover: Color::from_u32(0x58a6ff),
        }
    }

    pub fn github_dark() -> Self {
        Self {
            brand: Color::from_u32(0x58a6ff),
            ..Self::deep_ocean()
        }
    }

    pub fn style_primary(&self) -> Style {
        Style::default().fg(self.text_primary).bg(self.bg_base)
    }

    pub fn style_brand(&self) -> Style {
        Style::default().fg(self.brand)
    }

    pub fn style_user(&self) -> Style {
        Style::default().fg(self.user)
    }

    pub fn style_success(&self) -> Style {
        Style::default().fg(self.success)
    }

    pub fn style_error(&self) -> Style {
        Style::default().fg(self.error)
    }

    pub fn style_selection(&self) -> Style {
        Style::default().fg(self.selection_fg).bg(self.selection_bg)
    }

    pub fn style_muted(&self) -> Style {
        Style::default().fg(self.text_muted)
    }

    pub fn header_style(&self) -> Style {
        self.style_primary().bg(self.bg_surface)
    }

    pub fn drawer_style(&self) -> Style {
        self.style_primary().bg(self.bg_surface)
    }

    pub fn input_style(&self) -> Style {
        self.style_primary().bg(self.bg_surface)
    }

    pub fn status_bar_style(&self) -> Style {
        self.style_muted().bg(self.bg_base)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deep_ocean_colors() {
        let theme = Theme::deep_ocean();
        assert_eq!(theme.brand, Color::from_u32(0x39d0d8));
        assert_eq!(theme.user, Color::from_u32(0xf0883e));
        assert_eq!(theme.success, Color::from_u32(0x3fb950));
    }

    #[test]
    fn test_style_construction() {
        let theme = Theme::deep_ocean();
        let style = theme.style_brand();
        assert_eq!(style.fg, Some(theme.brand));
    }

    #[test]
    fn test_theme_presets() {
        let t1 = Theme::deep_ocean();
        let t2 = Theme::github_dark();
        assert_ne!(t1.brand, t2.brand);
    }
}
```

- [ ] **Step 2: 导出新模块**

修改 `src/tui/mod.rs`，在现有内容后添加：

```rust
pub mod theme;
```

- [ ] **Step 3: 编译验证**

Run: `cargo test --lib tui::theme`
Expected: 3 tests PASS

- [ ] **Step 4: Commit**

```bash
git add src/tui/theme.rs src/tui/mod.rs
git commit -m "feat(tui): add Theme Token system with presets"
```

---

### Task 2: AppEvent 枚举与 FocusArea

**Files:**
- Create: `src/tui/event.rs`
- Modify: `src/tui/mod.rs`

**Context:** 当前 `AppEvent` 在 `app.rs` 中定义，只有 `Tick`, `SseEvent`, `ChatComplete`, `ExecuteComplete`。需要扩展为全局事件系统。

- [ ] **Step 1: 创建 event.rs**

创建 `src/tui/event.rs`：

```rust
use crate::server::sse::SseEvent;

#[derive(Debug, Clone)]
pub enum AppEvent {
    Tick,
    Resize(u16, u16),
    ToggleLeftDrawer,
    ToggleRightDrawer,
    CloseDrawers,
    FocusNext,
    FocusPrev,
    SetFocus(FocusArea),
    ToggleModelDropdown,
    ToggleThemeDropdown,
    SelectModel(String),
    SelectTheme(usize),
    NewSession,
    NewSessionWithName(String),
    NewSessionFromTemplate(SessionTemplate),
    SubmitMessage(String),
    InputChanged(String),
    ScrollUp,
    ScrollDown,
    CopyLastCode,
    StopGeneration,
    SseEvent(SseEvent),
    ChatComplete,
    ExecuteComplete(String),
    SwitchSession(String),
    DeleteSession(String),
    RenameSession(String, String),
    ToggleFolder(String),
    SelectFile(String),
    OpenFile(String),
    PreviewFile(String),
    AddToContext(String),
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusArea {
    Header,
    Main,
    Input,
    LeftDrawer,
    RightDrawer,
}

#[derive(Debug, Clone)]
pub enum SessionTemplate {
    Empty,
    FromLastContext,
    CodeReview,
    Debug,
}
```

- [ ] **Step 2: 导出新模块**

修改 `src/tui/mod.rs`：

```rust
pub mod event;
pub mod theme;
```

- [ ] **Step 3: 编译验证**

Run: `cargo check`
Expected: 编译通过，无错误

- [ ] **Step 4: Commit**

```bash
git add src/tui/event.rs src/tui/mod.rs
git commit -m "feat(tui): add AppEvent enum and FocusArea"
```

---

### Task 3: Component Trait 定义

**Files:**
- Create: `src/tui/components/mod.rs`
- Create: `src/tui/components/` 目录 + 6 个骨架文件
- Modify: `src/tui/mod.rs`

**Context:** 定义所有 UI 组件的统一接口，创建骨架文件确保后续 Task 可以并行开发。

- [ ] **Step 1: 创建 Component trait 和目录结构**

创建 `src/tui/components/mod.rs`：

```rust
use crossterm::event::Event;
use ratatui::{layout::Rect, Frame};

use crate::tui::event::AppEvent;
use crate::tui::theme::Theme;

pub mod chat;
pub mod header;
pub mod input;
pub mod left_drawer;
pub mod right_drawer;
pub mod status_bar;

pub trait Component {
    fn draw(&self, frame: &mut Frame, area: Rect, theme: &Theme);
    fn handle_event(&mut self, event: &Event, focus: bool) -> Option<AppEvent>;
    fn update(&mut self, _event: &AppEvent) {}
    fn is_focusable(&self) -> bool {
        true
    }
}
```

- [ ] **Step 2: 创建 6 个骨架组件文件**

创建 `src/tui/components/header.rs`：

```rust
use crossterm::event::Event;
use ratatui::{layout::Rect, Frame};

use crate::tui::components::Component;
use crate::tui::event::AppEvent;
use crate::tui::theme::Theme;

pub struct Header {
    current_model: String,
}

impl Header {
    pub fn new() -> Self {
        Self {
            current_model: "unknown".to_string(),
        }
    }

    pub fn set_current_model(&mut self, model: String) {
        self.current_model = model;
    }

    pub fn set_session_id(&mut self, _id: String) {}
    pub fn session_id(&self) -> Option<String> { None }
    pub fn toggle_model_dropdown(&mut self) {}
    pub fn toggle_theme_dropdown(&mut self) {}
    pub fn on_tick(&mut self) {}
}

impl Component for Header {
    fn draw(&self, _frame: &mut Frame, _area: Rect, _theme: &Theme) {}
    fn handle_event(&mut self, _event: &Event, _focus: bool) -> Option<AppEvent> {
        None
    }
}
```

创建 `src/tui/components/left_drawer.rs`：

```rust
use crossterm::event::Event;
use ratatui::{layout::Rect, Frame};

use crate::tui::components::Component;
use crate::tui::event::AppEvent;
use crate::tui::theme::Theme;

pub struct LeftDrawer;

impl LeftDrawer {
    pub fn new() -> Self {
        Self
    }
}

impl Component for LeftDrawer {
    fn draw(&self, _frame: &mut Frame, _area: Rect, _theme: &Theme) {}
    fn handle_event(&mut self, _event: &Event, _focus: bool) -> Option<AppEvent> {
        None
    }
}
```

创建 `src/tui/components/right_drawer.rs`：

```rust
use crossterm::event::Event;
use ratatui::{layout::Rect, Frame};

use crate::tui::components::Component;
use crate::tui::event::AppEvent;
use crate::tui::theme::Theme;

pub struct RightDrawer;

impl RightDrawer {
    pub fn new() -> Self {
        Self
    }
}

impl Component for RightDrawer {
    fn draw(&self, _frame: &mut Frame, _area: Rect, _theme: &Theme) {}
    fn handle_event(&mut self, _event: &Event, _focus: bool) -> Option<AppEvent> {
        None
    }
}
```

创建 `src/tui/components/chat.rs`：

```rust
use crossterm::event::Event;
use ratatui::{layout::Rect, Frame};

use crate::tui::components::Component;
use crate::tui::event::AppEvent;
use crate::tui::theme::Theme;

pub struct Chat;

impl Chat {
    pub fn new() -> Self {
        Self
    }

    pub fn add_user_message(&mut self, _content: &str) {}
    pub fn on_tick(&mut self) {}
}

impl Component for Chat {
    fn draw(&self, _frame: &mut Frame, _area: Rect, _theme: &Theme) {}
    fn handle_event(&mut self, _event: &Event, _focus: bool) -> Option<AppEvent> {
        None
    }
}
```

创建 `src/tui/components/input.rs`：

```rust
use crossterm::event::Event;
use ratatui::{layout::Rect, Frame};

use crate::tui::components::Component;
use crate::tui::event::AppEvent;
use crate::tui::theme::Theme;

pub struct Input;

impl Input {
    pub fn new() -> Self {
        Self
    }

    pub fn visible_lines(&self) -> u16 {
        1
    }
}

impl Component for Input {
    fn draw(&self, _frame: &mut Frame, _area: Rect, _theme: &Theme) {}
    fn handle_event(&mut self, _event: &Event, _focus: bool) -> Option<AppEvent> {
        None
    }
}
```

创建 `src/tui/components/status_bar.rs`：

```rust
use crossterm::event::Event;
use ratatui::{layout::Rect, Frame};

use crate::tui::components::Component;
use crate::tui::event::AppEvent;
use crate::tui::theme::Theme;

pub struct StatusBar;

impl StatusBar {
    pub fn new() -> Self {
        Self
    }
}

impl Component for StatusBar {
    fn draw(&self, _frame: &mut Frame, _area: Rect, _theme: &Theme) {}
    fn handle_event(&mut self, _event: &Event, _focus: bool) -> Option<AppEvent> {
        None
    }
    fn is_focusable(&self) -> bool {
        false
    }
}
```

- [ ] **Step 3: 修改 tui/mod.rs**

```rust
pub mod components;
pub mod event;
pub mod theme;
```

- [ ] **Step 4: 编译验证**

Run: `cargo check`
Expected: 编译通过

- [ ] **Step 5: Commit**

```bash
git add src/tui/components/
git commit -m "feat(tui): add Component trait and component skeletons"
```

---

### Task 4: LayoutManager

**Files:**
- Create: `src/tui/layout.rs`
- Modify: `src/tui/mod.rs`

**Context:** 实现 1+2 抽屉系统的布局计算。

- [ ] **Step 1: 实现 LayoutManager**

创建 `src/tui/layout.rs`：

```rust
use ratatui::layout::Rect;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelState {
    None,
    LeftDrawer,
    RightDrawer,
}

pub struct LayoutManager {
    pub terminal_size: (u16, u16),
    pub panel: PanelState,
    pub narrow_mode: bool,
}

#[derive(Debug)]
pub struct LayoutAreas {
    pub header: Rect,
    pub left_drawer: Option<Rect>,
    pub main: Rect,
    pub right_drawer: Option<Rect>,
    pub status_bar: Rect,
    pub overlay: Option<Rect>,
}

impl LayoutManager {
    pub fn new(width: u16, height: u16) -> Self {
        let narrow_mode = width < 80;
        Self {
            terminal_size: (width, height),
            panel: PanelState::None,
            narrow_mode,
        }
    }

    pub fn resize(&mut self, width: u16, height: u16) {
        self.terminal_size = (width, height);
        self.narrow_mode = width < 80;
    }

    pub fn toggle_left(&mut self) {
        self.panel = match self.panel {
            PanelState::LeftDrawer => PanelState::None,
            _ => PanelState::LeftDrawer,
        };
    }

    pub fn toggle_right(&mut self) {
        self.panel = match self.panel {
            PanelState::RightDrawer => PanelState::None,
            _ => PanelState::RightDrawer,
        };
    }

    pub fn close_drawers(&mut self) {
        self.panel = PanelState::None;
    }

    pub fn calculate(&self) -> LayoutAreas {
        let (width, height) = self.terminal_size;
        let header_height = 3u16;
        let status_height = 1u16;
        let main_height = height.saturating_sub(header_height + status_height);

        if self.narrow_mode && self.panel != PanelState::None {
            let overlay_width = (width as f32 * 0.75).max(30.0).min(width as f32) as u16;
            let overlay_x = match self.panel {
                PanelState::LeftDrawer => 0,
                PanelState::RightDrawer => width.saturating_sub(overlay_width),
                PanelState::None => 0,
            };

            LayoutAreas {
                header: Rect::new(0, 0, width, header_height),
                main: Rect::new(0, header_height, width, main_height),
                status_bar: Rect::new(0, height - status_height, width, status_height),
                left_drawer: None,
                right_drawer: None,
                overlay: Some(Rect::new(overlay_x, header_height, overlay_width, main_height)),
            }
        } else {
            let drawer_width = ((width as f32 * 0.28) as u16).clamp(24, 40);
            let main_width = match self.panel {
                PanelState::None => width,
                _ => width.saturating_sub(drawer_width),
            };

            let (left_x, main_x, right_x) = match self.panel {
                PanelState::LeftDrawer => (0, drawer_width, width),
                PanelState::RightDrawer => (0, 0, main_width),
                PanelState::None => (0, 0, width),
            };

            LayoutAreas {
                header: Rect::new(0, 0, width, header_height),
                left_drawer: (self.panel == PanelState::LeftDrawer).then(|| {
                    Rect::new(left_x, header_height, drawer_width, main_height)
                }),
                main: Rect::new(main_x, header_height, main_width, main_height),
                right_drawer: (self.panel == PanelState::RightDrawer).then(|| {
                    Rect::new(right_x, header_height, drawer_width, main_height)
                }),
                status_bar: Rect::new(0, height - status_height, width, status_height),
                overlay: None,
            }
        }
    }

    pub fn split_main(main: Rect, input_lines: u16) -> (Rect, Rect) {
        let input_height = input_lines.clamp(1, 5) + 2;
        let messages_height = main.height.saturating_sub(input_height);

        let messages = Rect::new(main.x, main.y, main.width, messages_height);
        let input = Rect::new(main.x, main.y + messages_height, main.width, input_height);

        (messages, input)
    }
}

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
        assert!(areas.right_drawer.is_none());
        assert!(areas.overlay.is_none());
        assert_eq!(areas.main.width, 120);
    }

    #[test]
    fn test_left_drawer_expands() {
        let mut layout = LayoutManager::new(120, 30);
        layout.toggle_left();
        let areas = layout.calculate();

        assert!(areas.left_drawer.is_some());
        assert!(areas.right_drawer.is_none());
        assert!(areas.overlay.is_none());
        assert!(areas.main.width < 120);
    }

    #[test]
    fn test_drawer_mutual_exclusion() {
        let mut layout = LayoutManager::new(120, 30);
        layout.toggle_left();
        layout.toggle_right();

        assert_eq!(layout.panel, PanelState::RightDrawer);
        let areas = layout.calculate();
        assert!(areas.left_drawer.is_none());
        assert!(areas.right_drawer.is_some());
    }

    #[test]
    fn test_narrow_mode_overlay() {
        let mut layout = LayoutManager::new(60, 30);
        layout.toggle_left();
        let areas = layout.calculate();

        assert!(areas.overlay.is_some());
        assert!(areas.left_drawer.is_none());
        assert_eq!(areas.main.width, 60);
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
}
```

- [ ] **Step 2: 导出新模块**

修改 `src/tui/mod.rs`：

```rust
pub mod components;
pub mod event;
pub mod layout;
pub mod theme;
```

- [ ] **Step 3: 运行测试**

Run: `cargo test --lib tui::layout`
Expected: 5 tests PASS

- [ ] **Step 4: Commit**

```bash
git add src/tui/layout.rs src/tui/mod.rs
git commit -m "feat(tui): add LayoutManager with drawer mutual exclusion and narrow mode"
```


---

## Phase 2: App 核心重构

### Task 5: TuiApp 状态机与事件循环

**Files:**
- Modify: `src/tui/app.rs`（全面重写）

**Context:** 将现有 `TuiApp` 重构为基于 Component 的状态机。保留现有 SSE 聊天逻辑，但用新的 `AppEvent` 系统重新组织。

- [ ] **Step 1: 重写 app.rs**

替换 `src/tui/app.rs` 的全部内容：

```rust
use std::sync::Arc;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::DefaultTerminal;
use tokio::sync::mpsc;

use crate::server::sse::SseEvent;
use crate::tui::components::{
    chat::Chat, header::Header, input::Input, left_drawer::LeftDrawer,
    right_drawer::RightDrawer, status_bar::StatusBar, Component,
};
use crate::tui::event::{AppEvent, FocusArea};
use crate::tui::layout::{LayoutManager, PanelState};
use crate::tui::theme::Theme;

use super::client::TuiClient;

pub struct TuiApp {
    layout: LayoutManager,
    theme: Arc<Theme>,
    themes: Vec<Arc<Theme>>,
    theme_index: usize,

    header: Header,
    left_drawer: LeftDrawer,
    right_drawer: RightDrawer,
    chat: Chat,
    input: Input,
    status_bar: StatusBar,

    focus: FocusArea,
    is_generating: bool,
    should_quit: bool,

    client: TuiClient,
    event_tx: mpsc::Sender<AppEvent>,
    event_rx: mpsc::Receiver<AppEvent>,
}

impl TuiApp {
    pub fn new() -> Self {
        let (event_tx, event_rx) = mpsc::channel(100);
        let themes = vec![
            Arc::new(Theme::deep_ocean()),
            Arc::new(Theme::github_dark()),
        ];

        Self {
            layout: LayoutManager::new(80, 24),
            theme: themes[0].clone(),
            themes,
            theme_index: 0,
            header: Header::new(),
            left_drawer: LeftDrawer::new(),
            right_drawer: RightDrawer::new(),
            chat: Chat::new(),
            input: Input::new(),
            status_bar: StatusBar::new(),
            focus: FocusArea::Main,
            is_generating: false,
            should_quit: false,
            client: TuiClient::new(),
            event_tx,
            event_rx,
        }
    }

    pub async fn run(&mut self, terminal: &mut DefaultTerminal) -> anyhow::Result<()> {
        if let Ok(model) = self.client.get_status().await {
            self.header.set_current_model(model);
        }

        let mut interval = tokio::time::interval(Duration::from_millis(80));

        while !self.should_quit {
            terminal.draw(|frame| self.draw(frame))?;

            tokio::select! {
                _ = interval.tick() => {
                    self.handle_app_event(AppEvent::Tick).await;
                }
                Some(event) = self.event_rx.recv() => {
                    self.handle_app_event(event).await;
                }
                result = Self::read_crossterm_event() => {
                    if let Ok(event) = result {
                        self.route_event(event).await;
                    }
                }
            }
        }

        Ok(())
    }

    async fn read_crossterm_event() -> anyhow::Result<Event> {
        tokio::task::spawn_blocking(|| {
            if event::poll(Duration::from_millis(100))? {
                Ok(event::read()?)
            } else {
                Err(anyhow::anyhow!("timeout"))
            }
        })
        .await?
    }

    fn draw(&mut self, frame: &mut ratatui::Frame) {
        let areas = self.layout.calculate();
        let input_lines = self.input.visible_lines();
        let (messages_area, input_area) = LayoutManager::split_main(areas.main, input_lines);

        self.header.draw(frame, areas.header, &self.theme);
        self.chat.draw(frame, messages_area, &self.theme);
        self.input.draw(frame, input_area, &self.theme);
        self.status_bar.draw(frame, areas.status_bar, &self.theme);

        if let Some(overlay_area) = areas.overlay {
            let dim = ratatui::widgets::Block::default()
                .style(ratatui::style::Style::default().bg(self.theme.bg_overlay));
            frame.render_widget(dim, areas.main);

            match self.layout.panel {
                PanelState::LeftDrawer => {
                    self.left_drawer.draw(frame, overlay_area, &self.theme);
                }
                PanelState::RightDrawer => {
                    self.right_drawer.draw(frame, overlay_area, &self.theme);
                }
                _ => {}
            }
        } else {
            if let Some(area) = areas.left_drawer {
                self.left_drawer.draw(frame, area, &self.theme);
            }
            if let Some(area) = areas.right_drawer {
                self.right_drawer.draw(frame, area, &self.theme);
            }
        }
    }

    fn next_theme(&mut self) {
        self.theme_index = (self.theme_index + 1) % self.themes.len();
        self.theme = self.themes[self.theme_index].clone();
    }

    async fn route_event(&mut self, event: Event) {
        if let Event::Key(key) = event {
            if key.kind != KeyEventKind::Press {
                return;
            }

            match (key.modifiers, key.code) {
                (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
                    if self.is_generating {
                        self.handle_app_event(AppEvent::StopGeneration).await;
                    } else {
                        self.should_quit = true;
                    }
                    return;
                }
                (KeyModifiers::CONTROL, KeyCode::Char('b')) => {
                    self.layout.toggle_left();
                    self.focus = FocusArea::LeftDrawer;
                    return;
                }
                (KeyModifiers::CONTROL, KeyCode::Char('h')) => {
                    self.layout.toggle_right();
                    self.focus = FocusArea::RightDrawer;
                    return;
                }
                (KeyModifiers::CONTROL, KeyCode::Char('m')) => {
                    self.header.toggle_model_dropdown();
                    self.focus = FocusArea::Header;
                    return;
                }
                (KeyModifiers::CONTROL, KeyCode::Char('t')) => {
                    self.next_theme();
                    return;
                }
                (KeyModifiers::NONE, KeyCode::Esc) => {
                    if self.layout.panel != PanelState::None {
                        self.layout.close_drawers();
                    } else {
                        self.focus = FocusArea::Main;
                    }
                    return;
                }
                _ => {}
            }
        }

        let app_event = match self.focus {
            FocusArea::Header => self.header.handle_event(&event, true),
            FocusArea::LeftDrawer => self.left_drawer.handle_event(&event, true),
            FocusArea::RightDrawer => self.right_drawer.handle_event(&event, true),
            FocusArea::Main => self.chat.handle_event(&event, true),
            FocusArea::Input => self.input.handle_event(&event, true),
        };

        if let Some(app_event) = app_event {
            self.handle_app_event(app_event).await;
        }
    }

    async fn handle_app_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Tick => {
                self.chat.on_tick();
                self.header.on_tick();
            }
            AppEvent::Resize(w, h) => {
                self.layout.resize(w, h);
            }
            AppEvent::SubmitMessage(msg) => {
                self.is_generating = true;
                self.chat.add_user_message(&msg);
                self.start_chat_stream(msg).await;
            }
            AppEvent::SseEvent(sse_event) => {
                self.chat.handle_sse_event(&sse_event);
                if let SseEvent::Done { session_id } = &sse_event {
                    self.header.set_session_id(session_id.clone());
                }
            }
            AppEvent::ChatComplete => {
                self.is_generating = false;
            }
            AppEvent::StopGeneration => {
                self.is_generating = false;
            }
            _ => {}
        }

        self.header.update(&event);
        self.chat.update(&event);
        self.input.update(&event);
        self.left_drawer.update(&event);
        self.right_drawer.update(&event);
        self.status_bar.update(&event);
    }

    async fn start_chat_stream(&self, message: String) {
        let client = self.client.clone();
        let tx = self.event_tx.clone();
        let session_id = self.header.session_id().clone();

        tokio::spawn(async move {
            match client.chat(session_id, message, tx.clone()).await {
                Ok(_) => {
                    let _ = tx.send(AppEvent::ChatComplete).await;
                }
                Err(e) => {
                    let _ = tx.send(AppEvent::SseEvent(SseEvent::Error {
                        message: e.to_string(),
                    }))
                    .await;
                    let _ = tx.send(AppEvent::ChatComplete).await;
                }
            }
        });
    }
}
```

- [ ] **Step 2: 更新 Chat 组件接口**

修改 `src/tui/components/chat.rs`，添加 SSE 处理方法：

```rust
use crossterm::event::Event;
use ratatui::{layout::Rect, Frame};

use crate::server::sse::SseEvent;
use crate::tui::components::Component;
use crate::tui::event::AppEvent;
use crate::tui::theme::Theme;

pub struct Chat;

impl Chat {
    pub fn new() -> Self {
        Self
    }

    pub fn add_user_message(&mut self, _content: &str) {}
    pub fn on_tick(&mut self) {}
    pub fn handle_sse_event(&mut self, _event: &SseEvent) {}
}

impl Component for Chat {
    fn draw(&self, _frame: &mut Frame, _area: Rect, _theme: &Theme) {}
    fn handle_event(&mut self, _event: &Event, _focus: bool) -> Option<AppEvent> {
        None
    }
}
```

- [ ] **Step 3: 更新 Header 组件接口**

修改 `src/tui/components/header.rs`：

```rust
use crossterm::event::Event;
use ratatui::{layout::Rect, Frame};

use crate::tui::components::Component;
use crate::tui::event::AppEvent;
use crate::tui::theme::Theme;

pub struct Header {
    current_model: String,
    session_id: Option<String>,
}

impl Header {
    pub fn new() -> Self {
        Self {
            current_model: "unknown".to_string(),
            session_id: None,
        }
    }

    pub fn set_current_model(&mut self, model: String) {
        self.current_model = model;
    }

    pub fn set_session_id(&mut self, id: String) {
        self.session_id = Some(id);
    }

    pub fn session_id(&self) -> Option<String> {
        self.session_id.clone()
    }

    pub fn toggle_model_dropdown(&mut self) {}
    pub fn toggle_theme_dropdown(&mut self) {}
    pub fn on_tick(&mut self) {}
}

impl Component for Header {
    fn draw(&self, _frame: &mut Frame, _area: Rect, _theme: &Theme) {}
    fn handle_event(&mut self, _event: &Event, _focus: bool) -> Option<AppEvent> {
        None
    }
}
```

- [ ] **Step 4: 删除旧的 ui.rs**

```bash
rm src/tui/ui.rs
```

修改 `src/tui/mod.rs` 确保没有引用 `ui`：

```rust
pub mod app;
pub mod client;
pub mod components;
pub mod event;
pub mod layout;
pub mod theme;
```

- [ ] **Step 5: 编译验证**

Run: `cargo check`
Expected: 编译通过，无 ui.rs 相关错误

- [ ] **Step 6: Commit**

```bash
git add src/tui/app.rs src/tui/components/chat.rs src/tui/components/header.rs src/tui/mod.rs
git rm src/tui/ui.rs
git commit -m "refactor(tui): rewrite TuiApp with Component architecture and AppEvent system"
```

---

## Phase 3: P0 组件实现

### Task 6: Header 组件

**Files:**
- Modify: `src/tui/components/header.rs`

**Context:** Header 显示 Logo、当前模型、状态指示器。需要实现模型下拉菜单和主题下拉菜单的渲染与交互。

- [ ] **Step 1: 实现 Header 状态与渲染**

替换 `src/tui/components/header.rs`：

```rust
use crossterm::event::{Event, KeyCode, KeyEventKind};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Style, Modifier},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::tui::components::Component;
use crate::tui::event::AppEvent;
use crate::tui::theme::Theme;

#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub name: String,
    pub capabilities: Vec<String>,
}

pub struct Header {
    current_model: String,
    session_id: Option<String>,
    model_dropdown_open: bool,
    theme_dropdown_open: bool,
    dropdown_selected: usize,
    models: Vec<ModelInfo>,
    status: HeaderStatus,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HeaderStatus {
    Ready,
    Generating,
    Streaming,
}

impl Header {
    pub fn new() -> Self {
        Self {
            current_model: "unknown".to_string(),
            session_id: None,
            model_dropdown_open: false,
            theme_dropdown_open: false,
            dropdown_selected: 0,
            models: vec![],
            status: HeaderStatus::Ready,
        }
    }

    pub fn set_current_model(&mut self, model: String) {
        self.current_model = model;
    }

    pub fn set_session_id(&mut self, id: String) {
        self.session_id = Some(id);
    }

    pub fn session_id(&self) -> Option<String> {
        self.session_id.clone()
    }

    pub fn toggle_model_dropdown(&mut self) {
        self.model_dropdown_open = !self.model_dropdown_open;
        self.theme_dropdown_open = false;
        self.dropdown_selected = 0;
    }

    pub fn toggle_theme_dropdown(&mut self) {
        self.theme_dropdown_open = !self.theme_dropdown_open;
        self.model_dropdown_open = false;
        self.dropdown_selected = 0;
    }

    pub fn on_tick(&mut self) {}

    pub fn set_status(&mut self, status: HeaderStatus) {
        self.status = status;
    }
}

impl Component for Header {
    fn draw(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let block = Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(theme.border))
            .style(theme.header_style());

        let inner = block.inner(area);
        frame.render_widget(block, area);

        // Logo
        let logo = Span::styled("FiCode", theme.style_brand().add_modifier(Modifier::BOLD));

        // 模型指示
        let model_text = format!("▼ {}", self.current_model);
        let model = Span::styled(model_text, theme.style_primary());

        // 状态指示
        let (status_icon, status_color) = match self.status {
            HeaderStatus::Ready => ("●", theme.success),
            HeaderStatus::Generating => ("⟳", theme.warning),
            HeaderStatus::Streaming => ("⚡", theme.brand),
        };
        let status = Span::styled(
            format!("{} ready", status_icon),
            Style::default().fg(status_color),
        );

        let line = Line::from(vec![
            logo,
            Span::raw(" │ "),
            model,
            Span::raw(" │ "),
            status,
        ]);

        let paragraph = Paragraph::new(line).alignment(Alignment::Left);
        frame.render_widget(paragraph, inner);

        // 绘制下拉菜单
        if self.model_dropdown_open {
            self.draw_model_dropdown(frame, area, theme);
        }
    }

    fn handle_event(&mut self, event: &Event, _focus: bool) -> Option<AppEvent> {
        if let Event::Key(key) = event {
            if key.kind != KeyEventKind::Press {
                return None;
            }

            if self.model_dropdown_open {
                match key.code {
                    KeyCode::Up => {
                        if self.dropdown_selected > 0 {
                            self.dropdown_selected -= 1;
                        }
                        return Some(AppEvent::InputChanged(String::new()));
                    }
                    KeyCode::Down => {
                        if self.dropdown_selected < self.models.len().saturating_sub(1) {
                            self.dropdown_selected += 1;
                        }
                        return Some(AppEvent::InputChanged(String::new()));
                    }
                    KeyCode::Enter => {
                        if let Some(model) = self.models.get(self.dropdown_selected) {
                            let name = model.name.clone();
                            self.model_dropdown_open = false;
                            return Some(AppEvent::SelectModel(name));
                        }
                    }
                    KeyCode::Esc => {
                        self.model_dropdown_open = false;
                        return None;
                    }
                    _ => {}
                }
            }
        }
        None
    }
}

impl Header {
    fn draw_model_dropdown(&self, frame: &mut Frame, header_area: Rect, theme: &Theme) {
        let items: Vec<Line> = self
            .models
            .iter()
            .enumerate()
            .map(|(i, model)| {
                let prefix = if i == self.dropdown_selected {
                    "● "
                } else {
                    "  "
                };
                let style = if i == self.dropdown_selected {
                    theme.style_selection()
                } else {
                    theme.style_primary()
                };
                Line::styled(format!("{}{}", prefix, model.name), style)
            })
            .collect();

        let height = items.len().clamp(3, 10) as u16 + 2;
        let width = 30u16;
        let x = header_area.x + 10;
        let y = header_area.y + header_area.height;

        let area = Rect::new(x, y, width, height);
        frame.render_widget(Clear, area);

        let paragraph = Paragraph::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.border))
                    .style(theme.drawer_style()),
            );
        frame.render_widget(paragraph, area);
    }
}

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
    fn test_dropdown_toggle() {
        let mut header = Header::new();
        assert!(!header.model_dropdown_open);
        header.toggle_model_dropdown();
        assert!(header.model_dropdown_open);
        header.toggle_theme_dropdown();
        assert!(!header.model_dropdown_open);
        assert!(header.theme_dropdown_open);
    }
}
```

- [ ] **Step 2: 编译验证**

Run: `cargo test --lib tui::components::header`
Expected: 2 tests PASS

- [ ] **Step 3: Commit**

```bash
git add src/tui/components/header.rs
git commit -m "feat(tui): implement Header component with model dropdown"
```

---

### Task 7: Chat 组件

**Files:**
- Modify: `src/tui/components/chat.rs`

**Context:** 主聊天区需要渲染消息列表、支持滚动、显示代码块和 Loading 状态。

- [ ] **Step 1: 实现 Chat 组件**

替换 `src/tui/components/chat.rs`：

```rust
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::server::sse::SseEvent;
use crate::tui::components::Component;
use crate::tui::event::AppEvent;
use crate::tui::theme::Theme;

#[derive(Debug, Clone)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MessageRole {
    User,
    Assistant,
    System,
    Error,
}

pub struct Chat {
    messages: Vec<Message>,
    scroll_offset: usize,
    is_generating: bool,
    spinner_frame: usize,
}

const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

impl Chat {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            scroll_offset: 0,
            is_generating: false,
            spinner_frame: 0,
        }
    }

    pub fn add_user_message(&mut self, content: &str) {
        self.messages.push(Message {
            role: MessageRole::User,
            content: content.to_string(),
        });
    }

    pub fn on_tick(&mut self) {
        if self.is_generating {
            self.spinner_frame = (self.spinner_frame + 1) % SPINNER_FRAMES.len();
        }
    }

    pub fn handle_sse_event(&mut self, event: &SseEvent) {
        match event {
            SseEvent::Text { text } => {
                if let Some(last) = self.messages.last_mut() {
                    if last.role == MessageRole::Assistant {
                        last.content.push_str(text);
                    } else {
                        self.messages.push(Message {
                            role: MessageRole::Assistant,
                            content: text.clone(),
                        });
                    }
                } else {
                    self.messages.push(Message {
                        role: MessageRole::Assistant,
                        content: text.clone(),
                    });
                }
            }
            SseEvent::Error { message } => {
                self.messages.push(Message {
                    role: MessageRole::Error,
                    content: message.clone(),
                });
            }
            _ => {}
        }
    }

    pub fn set_generating(&mut self, generating: bool) {
        self.is_generating = generating;
        if !generating {
            self.spinner_frame = 0;
        }
    }
}

impl Component for Chat {
    fn draw(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border))
            .style(theme.style_primary());

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let mut lines: Vec<Line> = Vec::new();

        for msg in &self.messages {
            let (prefix, style) = match msg.role {
                MessageRole::User => ("You", theme.style_user().add_modifier(Modifier::BOLD)),
                MessageRole::Assistant => ("◆ AI", theme.style_brand().add_modifier(Modifier::BOLD)),
                MessageRole::System => ("ℹ️ ", Style::default().fg(theme.warning)),
                MessageRole::Error => ("❌ ", Style::default().fg(theme.error)),
            };

            lines.push(Line::from(vec![Span::styled(prefix, style)]));

            for text_line in msg.content.lines() {
                lines.push(Line::from(Span::styled(text_line, theme.style_primary())));
            }

            lines.push(Line::from(""));
        }

        if self.is_generating {
            let spinner = SPINNER_FRAMES[self.spinner_frame];
            lines.push(Line::from(vec![
                Span::styled("◆ AI ", theme.style_brand().add_modifier(Modifier::BOLD)),
                Span::styled(spinner, theme.style_brand()),
            ]));
        }

        let paragraph = Paragraph::new(Text::from(lines))
            .wrap(Wrap { trim: true })
            .scroll((self.scroll_offset as u16, 0));

        frame.render_widget(paragraph, inner);
    }

    fn handle_event(&mut self, event: &Event, _focus: bool) -> Option<AppEvent> {
        if let Event::Key(key) = event {
            if key.kind != KeyEventKind::Press {
                return None;
            }
            match (key.modifiers, key.code) {
                (KeyModifiers::CONTROL, KeyCode::Up) | (KeyModifiers::NONE, KeyCode::PageUp) => {
                    if self.scroll_offset > 0 {
                        self.scroll_offset -= 1;
                    }
                    return Some(AppEvent::ScrollUp);
                }
                (KeyModifiers::CONTROL, KeyCode::Down) | (KeyModifiers::NONE, KeyCode::PageDown) => {
                    self.scroll_offset += 1;
                    return Some(AppEvent::ScrollDown);
                }
                _ => {}
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_message() {
        let mut chat = Chat::new();
        chat.add_user_message("hello");
        assert_eq!(chat.messages.len(), 1);
        assert_eq!(chat.messages[0].role, MessageRole::User);
    }

    #[test]
    fn test_sse_text_appends() {
        let mut chat = Chat::new();
        chat.handle_sse_event(&SseEvent::Text {
            text: "Hello".to_string(),
        });
        chat.handle_sse_event(&SseEvent::Text {
            text: " world".to_string(),
        });
        assert_eq!(chat.messages.len(), 1);
        assert_eq!(chat.messages[0].content, "Hello world");
    }

    #[test]
    fn test_generating_state() {
        let mut chat = Chat::new();
        chat.set_generating(true);
        assert!(chat.is_generating);
        chat.on_tick();
        assert_eq!(chat.spinner_frame, 1);
    }
}
```

- [ ] **Step 2: 编译验证**

Run: `cargo test --lib tui::components::chat`
Expected: 3 tests PASS

- [ ] **Step 3: Commit**

```bash
git add src/tui/components/chat.rs
git commit -m "feat(tui): implement Chat component with message list and spinner"
```

---

### Task 8: Input 组件（多行输入）

**Files:**
- Modify: `src/tui/components/input.rs`

**Context:** 实现多行输入框，支持 Shift+Enter 换行、Slash 命令面板。

- [ ] **Step 1: 实现 Input 组件**

替换 `src/tui/components/input.rs`：

```rust
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    layout::Rect,
    style::{Style, Modifier},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::tui::components::Component;
use crate::tui::event::AppEvent;
use crate::tui::theme::Theme;

pub struct SlashCommand {
    pub name: String,
    pub description: String,
}

pub struct Input {
    content: String,
    cursor_position: usize,
    dropdown_visible: bool,
    dropdown_items: Vec<SlashCommand>,
    dropdown_selected: usize,
}

impl Input {
    pub fn new() -> Self {
        Self {
            content: String::new(),
            cursor_position: 0,
            dropdown_visible: false,
            dropdown_items: vec![
                SlashCommand {
                    name: "clear".to_string(),
                    description: "Clear conversation".to_string(),
                },
                SlashCommand {
                    name: "model".to_string(),
                    description: "Switch model".to_string(),
                },
                SlashCommand {
                    name: "file".to_string(),
                    description: "Attach file".to_string(),
                },
                SlashCommand {
                    name: "help".to_string(),
                    description: "Show help".to_string(),
                },
            ],
            dropdown_selected: 0,
        }
    }

    pub fn visible_lines(&self) -> u16 {
        let line_count = self.content.lines().count() as u16;
        line_count.clamp(1, 5)
    }

    fn insert_char(&mut self, c: char) {
        self.content.insert(self.cursor_position, c);
        self.cursor_position += c.len_utf8();
    }

    fn delete_char(&mut self) {
        if self.cursor_position > 0 {
            let prev_pos = self.content[..self.cursor_position]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.content.remove(prev_pos);
            self.cursor_position = prev_pos;
        }
    }

    fn check_slash_commands(&mut self) {
        if self.content == "/" {
            self.dropdown_visible = true;
            self.dropdown_selected = 0;
        } else if !self.content.starts_with('/') {
            self.dropdown_visible = false;
        }
    }
}

impl Component for Input {
    fn draw(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let placeholder = if self.content.is_empty() {
            "Type your message, or paste code..."
        } else {
            ""
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border))
            .style(theme.input_style());

        let inner = block.inner(area);
        frame.render_widget(block, area);

        if self.content.is_empty() {
            let text = Paragraph::new(placeholder).style(
                Style::default()
                    .fg(theme.text_placeholder)
                    .bg(theme.bg_surface),
            );
            frame.render_widget(text, inner);
        } else {
            let text = Paragraph::new(self.content.as_str())
                .style(theme.style_primary().bg(theme.bg_surface));
            frame.render_widget(text, inner);
        }

        // 绘制 slash 命令下拉
        if self.dropdown_visible && !self.dropdown_items.is_empty() {
            self.draw_dropdown(frame, area, theme);
        }
    }

    fn handle_event(&mut self, event: &Event, _focus: bool) -> Option<AppEvent> {
        if let Event::Key(key) = event {
            if key.kind != KeyEventKind::Press {
                return None;
            }

            // Slash 命令下拉导航
            if self.dropdown_visible {
                match key.code {
                    KeyCode::Up => {
                        if self.dropdown_selected > 0 {
                            self.dropdown_selected -= 1;
                        }
                        return None;
                    }
                    KeyCode::Down => {
                        if self.dropdown_selected < self.dropdown_items.len().saturating_sub(1) {
                            self.dropdown_selected += 1;
                        }
                        return None;
                    }
                    KeyCode::Enter => {
                        if let Some(cmd) = self.dropdown_items.get(self.dropdown_selected) {
                            self.content.clear();
                            self.cursor_position = 0;
                            self.dropdown_visible = false;
                            return match cmd.name.as_str() {
                                "clear" => Some(AppEvent::InputChanged(String::new())),
                                "model" => Some(AppEvent::ToggleModelDropdown),
                                _ => None,
                            };
                        }
                    }
                    KeyCode::Esc => {
                        self.dropdown_visible = false;
                        return None;
                    }
                    _ => {}
                }
            }

            match (key.modifiers, key.code) {
                (KeyModifiers::SHIFT, KeyCode::Enter) => {
                    self.insert_char('\n');
                    self.check_slash_commands();
                    return Some(AppEvent::InputChanged(self.content.clone()));
                }
                (KeyModifiers::NONE, KeyCode::Enter) => {
                    if !self.content.trim().is_empty() {
                        let msg = self.content.clone();
                        self.content.clear();
                        self.cursor_position = 0;
                        self.dropdown_visible = false;
                        return Some(AppEvent::SubmitMessage(msg));
                    }
                }
                (KeyModifiers::NONE, KeyCode::Char(c)) => {
                    self.insert_char(c);
                    self.check_slash_commands();
                    return Some(AppEvent::InputChanged(self.content.clone()));
                }
                (KeyModifiers::NONE, KeyCode::Backspace) => {
                    self.delete_char();
                    if self.content.is_empty() {
                        self.dropdown_visible = false;
                    }
                    return Some(AppEvent::InputChanged(self.content.clone()));
                }
                _ => {}
            }
        }
        None
    }
}

impl Input {
    fn draw_dropdown(&self, frame: &mut Frame, input_area: Rect, theme: &Theme) {
        let items: Vec<Line> = self
            .dropdown_items
            .iter()
            .enumerate()
            .map(|(i, cmd)| {
                let style = if i == self.dropdown_selected {
                    theme.style_selection()
                } else {
                    theme.style_primary()
                };
                Line::from(vec![
                    Span::styled(format!("/{}", cmd.name), style.add_modifier(Modifier::BOLD)),
                    Span::styled(format!(" - {}", cmd.description), style),
                ])
            })
            .collect();

        let height = items.len() as u16 + 2;
        let width = 40u16.min(input_area.width);
        let x = input_area.x;
        let y = input_area.y.saturating_sub(height);

        let area = Rect::new(x, y, width, height);

        let paragraph = Paragraph::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.border))
                    .style(theme.drawer_style()),
            );
        frame.render_widget(paragraph, area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_and_delete() {
        let mut input = Input::new();
        input.insert_char('h');
        input.insert_char('i');
        assert_eq!(input.content, "hi");
        assert_eq!(input.cursor_position, 2);

        input.delete_char();
        assert_eq!(input.content, "h");
        assert_eq!(input.cursor_position, 1);
    }

    #[test]
    fn test_multiline_lines() {
        let mut input = Input::new();
        input.insert_char('a');
        input.insert_char('\n');
        input.insert_char('b');
        assert_eq!(input.visible_lines(), 2);
    }

    #[test]
    fn test_slash_command_detection() {
        let mut input = Input::new();
        input.insert_char('/');
        input.check_slash_commands();
        assert!(input.dropdown_visible);

        input.content.clear();
        input.check_slash_commands();
        assert!(!input.dropdown_visible);
    }
}
```

- [ ] **Step 2: 编译验证**

Run: `cargo test --lib tui::components::input`
Expected: 3 tests PASS

- [ ] **Step 3: Commit**

```bash
git add src/tui/components/input.rs
git commit -m "feat(tui): implement Input component with multiline and slash commands"
```

---

### Task 9: StatusBar 组件

**Files:**
- Modify: `src/tui/components/status_bar.rs`

**Context:** 底部状态栏显示动态快捷键提示。

- [ ] **Step 1: 实现 StatusBar**

替换 `src/tui/components/status_bar.rs`：

```rust
use crossterm::event::Event;
use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::tui::components::Component;
use crate::tui::event::AppEvent;
use crate::tui::layout::PanelState;
use crate::tui::theme::Theme;

pub struct StatusBar {
    is_generating: bool,
    panel: PanelState,
}

impl StatusBar {
    pub fn new() -> Self {
        Self {
            is_generating: false,
            panel: PanelState::None,
        }
    }

    pub fn set_generating(&mut self, generating: bool) {
        self.is_generating = generating;
    }

    pub fn set_panel(&mut self, panel: PanelState) {
        self.panel = panel;
    }
}

impl Component for StatusBar {
    fn draw(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let mut spans = vec![];

        // Files toggle
        let files_label = match self.panel {
            PanelState::LeftDrawer => "[Ctrl+B] Hide",
            _ => "[Ctrl+B] Files",
        };
        spans.push(Span::styled(files_label, theme.style_muted()));
        spans.push(Span::raw("  "));

        // History toggle
        let history_label = match self.panel {
            PanelState::RightDrawer => "[Ctrl+H] Hide",
            _ => "[Ctrl+H] History",
        };
        spans.push(Span::styled(history_label, theme.style_muted()));
        spans.push(Span::raw("  "));

        // Model
        spans.push(Span::styled("[Ctrl+M] Model", theme.style_muted()));
        spans.push(Span::raw("  "));

        // Theme
        spans.push(Span::styled("[Ctrl+T] Theme", theme.style_muted()));
        spans.push(Span::raw("  "));

        // New session
        spans.push(Span::styled("[Ctrl+N] New", theme.style_muted()));

        // Stop generation
        if self.is_generating {
            spans.push(Span::raw("  "));
            spans.push(Span::styled(
                "[Ctrl+C] Stop",
                Style::default().fg(theme.error),
            ));
        }

        let line = Line::from(spans);
        let paragraph = Paragraph::new(line).style(theme.status_bar_style());
        frame.render_widget(paragraph, area);
    }

    fn handle_event(&mut self, _event: &Event, _focus: bool) -> Option<AppEvent> {
        None
    }

    fn is_focusable(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_bar_state() {
        let mut bar = StatusBar::new();
        assert!(!bar.is_generating);
        bar.set_generating(true);
        assert!(bar.is_generating);
    }
}
```

- [ ] **Step 2: 编译验证**

Run: `cargo test --lib tui::components::status_bar`
Expected: 1 test PASS

- [ ] **Step 3: Commit**

```bash
git add src/tui/components/status_bar.rs
git commit -m "feat(tui): implement StatusBar component with dynamic shortcuts"
```

---

## Phase 4: 后端 API 扩展

### Task 10: 后端 Session API

**Files:**
- Create: `src/server/models.rs`
- Create: `src/server/session_api.rs`
- Modify: `src/server/mod.rs`
- Modify: `src/server/server.rs`

**Context:** 在现有 Axum Server 上添加会话管理的 REST API，复用现有的 `SessionManager`。

- [ ] **Step 1: 创建 API DTO**

创建 `src/server/models.rs`：

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            code: None,
        }
    }

    pub fn error(message: impl Into<String>, code: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message.into()),
            code: Some(code.into()),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct SessionListResponse {
    pub sessions: Vec<SessionDto>,
    pub current_session_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SessionDto {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub last_active: String,
    pub message_count: usize,
    pub is_current: bool,
}

#[derive(Debug, Deserialize)]
pub struct CreateSessionRequest {
    pub name: String,
    #[serde(default)]
    pub template: String,
}

#[derive(Debug, Deserialize)]
pub struct RenameSessionRequest {
    pub name: String,
}
```

- [ ] **Step 2: 创建 Session API Handler**

创建 `src/server/session_api.rs`：

```rust
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

use crate::server::models::{
    ApiResponse, CreateSessionRequest, RenameSessionRequest, SessionDto, SessionListResponse,
};
use crate::server::server::AppState;

pub async fn list_sessions(State(state): State<AppState>) -> Json<ApiResponse<SessionListResponse>> {
    // 从 SessionManager 获取会话列表
    let sessions = vec![]; // 初始为空列表，实际实现中从 SessionManager 加载

    let response = SessionListResponse {
        sessions,
        current_session_id: None,
    };

    Json(ApiResponse::success(response))
}

pub async fn create_session(
    State(_state): State<AppState>,
    Json(req): Json<CreateSessionRequest>,
) -> Json<ApiResponse<SessionDto>> {
    let session = SessionDto {
        id: ulid::Ulid::new().to_string(),
        name: req.name,
        created_at: chrono::Utc::now().to_rfc3339(),
        last_active: chrono::Utc::now().to_rfc3339(),
        message_count: 0,
        is_current: true,
    };

    Json(ApiResponse::success(session))
}

pub async fn rename_session(
    State(_state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<RenameSessionRequest>,
) -> Json<ApiResponse<SessionDto>> {
    let session = SessionDto {
        id,
        name: req.name,
        created_at: chrono::Utc::now().to_rfc3339(),
        last_active: chrono::Utc::now().to_rfc3339(),
        message_count: 0,
        is_current: false,
    };

    Json(ApiResponse::success(session))
}

pub async fn delete_session(
    State(_state): State<AppState>,
    Path(_id): Path<String>,
) -> StatusCode {
    StatusCode::NO_CONTENT
}

pub async fn switch_session(
    State(_state): State<AppState>,
    Path(id): Path<String>,
) -> Json<ApiResponse<SessionDto>> {
    let session = SessionDto {
        id,
        name: "switched".to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
        last_active: chrono::Utc::now().to_rfc3339(),
        message_count: 0,
        is_current: true,
    };

    Json(ApiResponse::success(session))
}
```

- [ ] **Step 3: 注册路由**

修改 `src/server/mod.rs`：

```rust
pub mod models;
pub mod rpc;
pub mod server;
pub mod session;
pub mod session_api;
pub mod sse;

pub use server::Server;
```

修改 `src/server/server.rs`，在路由中添加新端点：

```rust
use axum::routing::{delete, get, post, put};
```

修改 `Server::run` 中的路由注册：

```rust
let app = Router::new()
    .route("/rpc", post(handle_rpc_endpoint))
    .route("/chat", post(handle_chat_endpoint))
    .route("/api/sessions", get(session_api::list_sessions).post(session_api::create_session))
    .route(
        "/api/sessions/:id",
        put(session_api::rename_session).delete(session_api::delete_session),
    )
    .route("/api/sessions/:id/switch", post(session_api::switch_session))
    .layer(cors_layer(self.state.config.clone()))
    .with_state(self.state.clone());
```

- [ ] **Step 4: 编译验证**

Run: `cargo check`
Expected: 编译通过

- [ ] **Step 5: Commit**

```bash
git add src/server/models.rs src/server/session_api.rs src/server/mod.rs src/server/server.rs
git commit -m "feat(server): add Session REST API endpoints"
```

---

### Task 11: TuiClient 扩展

**Files:**
- Modify: `src/tui/client.rs`

**Context:** 为 TuiClient 添加新的 REST API 调用方法。

- [ ] **Step 1: 扩展 TuiClient**

在 `src/tui/client.rs` 中添加新方法和类型：

```rust
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct SessionInfo {
    pub id: String,
    pub name: String,
    pub message_count: usize,
}

#[derive(Debug, Deserialize)]
pub struct SessionListResult {
    pub sessions: Vec<SessionInfo>,
    pub current_session_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}
```

在 `impl TuiClient` 中添加：

```rust
impl TuiClient {
    // ... 现有方法 ...

    pub async fn list_sessions(&self) -> anyhow::Result<SessionListResult> {
        let resp = self
            .client
            .get(format!("{}/api/sessions", self.base_url))
            .send()
            .await?
            .json::<ApiResponse<SessionListResult>>()
            .await?;

        match resp.data {
            Some(data) => Ok(data),
            None => Err(anyhow::anyhow!(resp.error.unwrap_or_default())),
        }
    }

    pub async fn create_session(&self, name: &str) -> anyhow::Result<SessionInfo> {
        let body = serde_json::json!({"name": name});
        let resp = self
            .client
            .post(format!("{}/api/sessions", self.base_url))
            .json(&body)
            .send()
            .await?
            .json::<ApiResponse<SessionInfo>>()
            .await?;

        match resp.data {
            Some(data) => Ok(data),
            None => Err(anyhow::anyhow!(resp.error.unwrap_or_default())),
        }
    }

    pub async fn switch_session(&self, id: &str) -> anyhow::Result<SessionInfo> {
        let resp = self
            .client
            .post(format!("{}/api/sessions/{}/switch", self.base_url, id))
            .send()
            .await?
            .json::<ApiResponse<SessionInfo>>()
            .await?;

        match resp.data {
            Some(data) => Ok(data),
            None => Err(anyhow::anyhow!(resp.error.unwrap_or_default())),
        }
    }
}
```

- [ ] **Step 2: 编译验证**

Run: `cargo check`
Expected: 编译通过

- [ ] **Step 3: Commit**

```bash
git add src/tui/client.rs
git commit -m "feat(tui): extend TuiClient with Session API methods"
```

---

## Phase 5: P1 组件与后端完善

### Task 12: RightDrawer（会话历史）

**Files:**
- Modify: `src/tui/components/right_drawer.rs`

**Context:** 实现右侧会话历史抽屉，支持列表展示、切换、过滤。

- [ ] **Step 1: 实现 RightDrawer**

替换 `src/tui/components/right_drawer.rs`：

```rust
use crossterm::event::{Event, KeyCode, KeyEventKind};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::tui::components::Component;
use crate::tui::event::AppEvent;
use crate::tui::theme::Theme;

#[derive(Debug, Clone)]
pub struct SessionMeta {
    pub id: String,
    pub name: String,
    pub last_active: String,
    pub message_count: usize,
    pub is_current: bool,
}

pub struct RightDrawer {
    sessions: Vec<SessionMeta>,
    selected_index: usize,
    filter: String,
    filter_active: bool,
}

impl RightDrawer {
    pub fn new() -> Self {
        Self {
            sessions: Vec::new(),
            selected_index: 0,
            filter: String::new(),
            filter_active: false,
        }
    }

    pub fn set_sessions(&mut self, sessions: Vec<SessionMeta>) {
        self.sessions = sessions;
        self.selected_index = 0;
    }
}

impl Component for RightDrawer {
    fn draw(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border))
            .title("Session History")
            .style(theme.drawer_style());

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let items: Vec<Line> = self
            .sessions
            .iter()
            .enumerate()
            .map(|(i, session)| {
                let prefix = if session.is_current { "● " } else { "○ " };
                let style = if i == self.selected_index {
                    theme.style_selection()
                } else if session.is_current {
                    theme.style_brand()
                } else {
                    theme.style_primary()
                };

                Line::from(vec![
                    Span::styled(prefix, style.add_modifier(Modifier::BOLD)),
                    Span::styled(&session.name, style),
                    Span::styled(
                        format!(" ({} msgs)", session.message_count),
                        theme.style_muted(),
                    ),
                ])
            })
            .collect();

        let paragraph = Paragraph::new(items);
        frame.render_widget(paragraph, inner);
    }

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
                    if self.selected_index < self.sessions.len().saturating_sub(1) {
                        self.selected_index += 1;
                    }
                    None
                }
                KeyCode::Enter => {
                    if let Some(session) = self.sessions.get(self.selected_index) {
                        return Some(AppEvent::SwitchSession(session.id.clone()));
                    }
                    None
                }
                _ => None,
            }
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_navigation() {
        let mut drawer = RightDrawer::new();
        drawer.set_sessions(vec![
            SessionMeta {
                id: "1".to_string(),
                name: "test1".to_string(),
                last_active: "".to_string(),
                message_count: 5,
                is_current: true,
            },
            SessionMeta {
                id: "2".to_string(),
                name: "test2".to_string(),
                last_active: "".to_string(),
                message_count: 3,
                is_current: false,
            },
        ]);

        assert_eq!(drawer.selected_index, 0);
    }
}
```

- [ ] **Step 2: 编译验证**

Run: `cargo test --lib tui::components::right_drawer`
Expected: 1 test PASS

- [ ] **Step 3: Commit**

```bash
git add src/tui/components/right_drawer.rs
git commit -m "feat(tui): implement RightDrawer with session list"
```

---

### Task 13: 后端 File API

**Files:**
- Create: `src/server/file_api.rs`
- Modify: `src/server/mod.rs`
- Modify: `src/server/server.rs`
- Modify: `Cargo.toml`

**Context:** 添加文件树和内容读取 API。

- [ ] **Step 1: 添加 walkdir 依赖**

修改 `Cargo.toml`，在 `[dependencies]` 下添加：

```toml
walkdir = "2.5"
```

- [ ] **Step 2: 创建 File API**

创建 `src/server/file_api.rs`：

```rust
use axum::{
    extract::{Query, State},
    Json,
};
use serde::Deserialize;
use std::collections::HashMap;

use crate::server::models::ApiResponse;
use crate::server::server::AppState;

#[derive(Debug, Deserialize)]
pub struct FileTreeQuery {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub git_status: bool,
}

#[derive(Debug, Deserialize)]
pub struct FileContentQuery {
    pub path: String,
}

#[derive(Debug, serde::Serialize)]
pub struct FileEntry {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub depth: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<FileEntry>>,
}

#[derive(Debug, serde::Serialize)]
pub struct FileTreeResponse {
    pub root: String,
    pub entries: Vec<FileEntry>,
}

#[derive(Debug, serde::Serialize)]
pub struct FileContentResponse {
    pub path: String,
    pub content: String,
    pub language: String,
    pub size: usize,
    pub lines: usize,
}

pub async fn file_tree(
    State(_state): State<AppState>,
    Query(query): Query<FileTreeQuery>,
) -> Json<ApiResponse<FileTreeResponse>> {
    let root = if query.path.is_empty() {
        ".".to_string()
    } else {
        query.path
    };

    let mut entries = Vec::new();

    if let Ok(dir) = std::fs::read_dir(&root) {
        for entry in dir.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let path = entry.path().to_string_lossy().to_string();
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);

            entries.push(FileEntry {
                path,
                name,
                is_dir,
                depth: 0,
                git_status: None,
                children: None,
            });
        }
    }

    let response = FileTreeResponse { root, entries };
    Json(ApiResponse::success(response))
}

pub async fn file_content(
    State(_state): State<AppState>,
    Query(query): Query<FileContentQuery>,
) -> Json<ApiResponse<FileContentResponse>> {
    match std::fs::read_to_string(&query.path) {
        Ok(content) => {
            let size = content.len();
            let lines = content.lines().count();
            let language = guess_language(&query.path);

            let response = FileContentResponse {
                path: query.path,
                content,
                language,
                size,
                lines,
            };
            Json(ApiResponse::success(response))
        }
        Err(e) => Json(ApiResponse::error(
            format!("Failed to read file: {}", e),
            "FILE_READ_ERROR",
        )),
    }
}

fn guess_language(path: &str) -> String {
    match path.rsplit('.').next() {
        Some("rs") => "rust".to_string(),
        Some("py") => "python".to_string(),
        Some("js") => "javascript".to_string(),
        Some("ts") => "typescript".to_string(),
        Some("md") => "markdown".to_string(),
        Some("json") => "json".to_string(),
        Some("yaml") | Some("yml") => "yaml".to_string(),
        _ => "text".to_string(),
    }
}
```

- [ ] **Step 3: 注册路由**

修改 `src/server/mod.rs`：

```rust
pub mod file_api;
pub mod models;
pub mod rpc;
pub mod server;
pub mod session;
pub mod session_api;
pub mod sse;

pub use server::Server;
```

修改 `src/server/server.rs` 中的路由：

```rust
let app = Router::new()
    .route("/rpc", post(handle_rpc_endpoint))
    .route("/chat", post(handle_chat_endpoint))
    .route("/api/sessions", get(session_api::list_sessions).post(session_api::create_session))
    .route(
        "/api/sessions/:id",
        put(session_api::rename_session).delete(session_api::delete_session),
    )
    .route("/api/sessions/:id/switch", post(session_api::switch_session))
    .route("/api/files", get(file_api::file_tree))
    .route("/api/files/content", get(file_api::file_content))
    .layer(cors_layer(self.state.config.clone()))
    .with_state(self.state.clone());
```

- [ ] **Step 4: 编译验证**

Run: `cargo check`
Expected: 编译通过

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml src/server/file_api.rs src/server/mod.rs src/server/server.rs
git commit -m "feat(server): add File REST API endpoints"
```

---

### Task 14: LeftDrawer（文件导航）

**Files:**
- Modify: `src/tui/components/left_drawer.rs`
- Modify: `src/tui/client.rs`

**Context:** 实现左侧文件导航抽屉，展示文件树。

- [ ] **Step 1: 扩展 TuiClient**

在 `src/tui/client.rs` 中添加文件 API 类型和方法：

```rust
#[derive(Debug, Deserialize)]
pub struct FileEntry {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub depth: usize,
}

#[derive(Debug, Deserialize)]
pub struct FileTreeResult {
    pub root: String,
    pub entries: Vec<FileEntry>,
}

// 在 impl TuiClient 中添加
pub async fn get_file_tree(&self, path: &str) -> anyhow::Result<FileTreeResult> {
    let resp = self
        .client
        .get(format!("{}/api/files", self.base_url))
        .query(&[("path", path)])
        .send()
        .await?
        .json::<ApiResponse<FileTreeResult>>()
        .await?;

    match resp.data {
        Some(data) => Ok(data),
        None => Err(anyhow::anyhow!(resp.error.unwrap_or_default())),
    }
}
```

- [ ] **Step 2: 实现 LeftDrawer**

替换 `src/tui/components/left_drawer.rs`：

```rust
use crossterm::event::{Event, KeyCode, KeyEventKind};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::tui::components::Component;
use crate::tui::event::AppEvent;
use crate::tui::theme::Theme;

#[derive(Debug, Clone)]
pub struct FileNode {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub depth: usize,
}

pub struct LeftDrawer {
    files: Vec<FileNode>,
    selected_index: usize,
    expanded_folders: std::collections::HashSet<String>,
}

impl LeftDrawer {
    pub fn new() -> Self {
        Self {
            files: Vec::new(),
            selected_index: 0,
            expanded_folders: std::collections::HashSet::new(),
        }
    }

    pub fn set_files(&mut self, files: Vec<FileNode>) {
        self.files = files;
        self.selected_index = 0;
    }
}

impl Component for LeftDrawer {
    fn draw(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border))
            .title("Files")
            .style(theme.drawer_style());

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let items: Vec<Line> = self
            .files
            .iter()
            .enumerate()
            .map(|(i, file)| {
                let indent = "  ".repeat(file.depth);
                let icon = if file.is_dir { "📁 " } else { "📄 " };
                let style = if i == self.selected_index {
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
    }

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
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_navigation() {
        let mut drawer = LeftDrawer::new();
        drawer.set_files(vec![
            FileNode {
                path: "src".to_string(),
                name: "src".to_string(),
                is_dir: true,
                depth: 0,
            },
            FileNode {
                path: "Cargo.toml".to_string(),
                name: "Cargo.toml".to_string(),
                is_dir: false,
                depth: 0,
            },
        ]);

        assert_eq!(drawer.selected_index, 0);
    }
}
```

- [ ] **Step 3: 编译验证**

Run: `cargo test --lib tui::components::left_drawer`
Expected: 1 test PASS

- [ ] **Step 4: Commit**

```bash
git add src/tui/components/left_drawer.rs src/tui/client.rs
git commit -m "feat(tui): implement LeftDrawer with file tree navigation"
```

---

## Phase 6: 集成与验证

### Task 15: App 状态同步完善

**Files:**
- Modify: `src/tui/app.rs`

**Context:** 完善 App 中的状态同步逻辑，将组件与后端 API 连接起来。

- [ ] **Step 1: 完善 handle_app_event**

在 `src/tui/app.rs` 中扩展 `handle_app_event` 方法，添加以下事件处理：

```rust
AppEvent::ToggleLeftDrawer => {
    self.layout.toggle_left();
    if self.layout.panel == PanelState::LeftDrawer {
        self.focus = FocusArea::LeftDrawer;
        // 加载文件树
        let client = self.client.clone();
        tokio::spawn(async move {
            if let Ok(tree) = client.get_file_tree(".").await {
                // 文件树数据将在后续迭代中通过状态同步机制更新到 LeftDrawer
                let _ = tree;
            }
        });
    }
}
AppEvent::ToggleRightDrawer => {
    self.layout.toggle_right();
    if self.layout.panel == PanelState::RightDrawer {
        self.focus = FocusArea::RightDrawer;
    }
}
AppEvent::SwitchSession(id) => {
    let client = self.client.clone();
    let tx = self.event_tx.clone();
    tokio::spawn(async move {
        match client.switch_session(&id).await {
            Ok(_) => {
                let _ = tx.send(AppEvent::ChatComplete).await;
            }
            Err(_) => {}
        }
    });
}
AppEvent::SelectModel(model) => {
    self.header.set_current_model(model);
}
AppEvent::SelectTheme(index) => {
    if index < self.themes.len() {
        self.theme_index = index;
        self.theme = self.themes[index].clone();
    }
}
```

- [ ] **Step 2: 编译验证**

Run: `cargo check`
Expected: 编译通过

- [ ] **Step 3: Commit**

```bash
git add src/tui/app.rs
git commit -m "feat(tui): wire up App event handling with backend APIs"
```

---

### Task 16: 端到端编译与测试回归

**Files:**
- All modified files

**Context:** 确保整个项目编译通过，现有测试不受影响。

- [ ] **Step 1: 完整编译**

Run: `cargo build`
Expected: 0 errors, 0 warnings

- [ ] **Step 2: 运行所有测试**

Run: `cargo test`
Expected: 所有现有测试 PASS + 新增测试 PASS

- [ ] **Step 3: Clippy 检查**

Run: `cargo clippy -- -D warnings`
Expected: 无警告

- [ ] **Step 4: 格式检查**

Run: `cargo fmt -- --check`
Expected: 无格式问题

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat(tui): complete IDE framework refactor with all components"
```

---

## 附录：快速参考

### 启动后端服务测试

```bash
cargo run -- server
# 在另一个终端测试 API
curl http://localhost:4040/api/sessions
curl "http://localhost:4040/api/files?path=."
```

### 启动 TUI 模式

```bash
cargo run
# 默认进入 TUI 模式
```

### 测试特定模块

```bash
cargo test --lib tui::theme
cargo test --lib tui::layout
cargo test --lib tui::components::header
cargo test --lib tui::components::chat
cargo test --lib tui::components::input
cargo test --lib tui::components::status_bar
cargo test --lib tui::components::right_drawer
cargo test --lib tui::components::left_drawer
```
