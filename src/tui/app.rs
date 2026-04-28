// MIT License
// Copyright (c) 2025 fi-code contributors
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use std::sync::Arc;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::DefaultTerminal;
use tokio::sync::mpsc;

use crate::server::sse::SseEvent;
use crate::tui::components::{
    chat::Chat, header::Header, input::Input, left_drawer::LeftDrawer, right_drawer::RightDrawer,
    status_bar::StatusBar, Component,
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

        let (term_w, term_h) = crossterm::terminal::size().unwrap_or((80, 24));

        Self {
            layout: LayoutManager::new(term_w, term_h),
            theme: themes[0].clone(),
            themes,
            theme_index: 0,
            header: Header::new(),
            left_drawer: LeftDrawer::new(),
            right_drawer: RightDrawer::new(),
            chat: Chat::new(),
            input: Input::new(),
            status_bar: StatusBar::new(),
            focus: FocusArea::Input,
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
                        match &event {
                            Event::Resize(w, h) => {
                                self.handle_app_event(AppEvent::Resize(*w, *h)).await;
                            }
                            _ => {
                                self.route_event(event).await;
                            }
                        }
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
        let area = frame.area();
        self.layout.resize(area.width, area.height);
        let areas = self.layout.calculate();
        let input_lines = self.input.visible_lines();
        // 如果有会话 ID，给输入框额外加一行显示
        let input_extra = if self.header.session_id().is_some() { 1 } else { 0 };
        let (messages_area, input_area) = LayoutManager::split_main(areas.main, input_lines + input_extra);

        self.header.draw(frame, areas.header, &self.theme, self.focus == FocusArea::Header);
        self.chat.draw(frame, messages_area, &self.theme, self.focus == FocusArea::Main);
        self.input.draw(frame, input_area, &self.theme, self.focus == FocusArea::Input);
        self.status_bar.draw(frame, areas.status_bar, &self.theme, false);

        if let Some(overlay_area) = areas.overlay {
            let dim = ratatui::widgets::Block::default()
                .style(ratatui::style::Style::default().bg(self.theme.bg_overlay));
            frame.render_widget(dim, areas.main);

            match self.layout.panel {
                PanelState::LeftDrawer => {
                    self.left_drawer.draw(frame, overlay_area, &self.theme, self.focus == FocusArea::LeftDrawer);
                }
                PanelState::RightDrawer => {
                    self.right_drawer.draw(frame, overlay_area, &self.theme, self.focus == FocusArea::RightDrawer);
                }
                _ => {}
            }
        } else {
            if let Some(area) = areas.left_drawer {
                self.left_drawer.draw(frame, area, &self.theme, self.focus == FocusArea::LeftDrawer);
            }
            if let Some(area) = areas.right_drawer {
                self.right_drawer.draw(frame, area, &self.theme, self.focus == FocusArea::RightDrawer);
            }
        }
    }

    fn next_theme(&mut self) {
        self.theme_index = (self.theme_index + 1) % self.themes.len();
        self.theme = self.themes[self.theme_index].clone();
    }

    fn cycle_focus(&mut self, forward: bool) {
        let areas = match self.layout.panel {
            PanelState::None => vec![
                FocusArea::Main,
                FocusArea::Input,
            ],
            PanelState::LeftDrawer => vec![
                FocusArea::LeftDrawer,
                FocusArea::Main,
                FocusArea::Input,
            ],
            PanelState::RightDrawer => vec![
                FocusArea::Main,
                FocusArea::Input,
                FocusArea::RightDrawer,
            ],
        };

        let current_idx = areas.iter().position(|a| a == &self.focus).unwrap_or(0);
        let next_idx = if forward {
            (current_idx + 1) % areas.len()
        } else {
            (current_idx + areas.len() - 1) % areas.len()
        };

        self.focus = areas[next_idx];
    }

    async fn route_event(&mut self, event: Event) {
        if let Event::Key(key) = event {
            if key.kind != KeyEventKind::Press {
                return;
            }

            // === 全局 Ctrl+字母快捷键 ===
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                if let KeyCode::Char(c) = key.code {
                    // 将控制字符（如 \x14）转换回字母（如 't'）
                    let lower = if c.is_ascii_control() {
                        (c as u8 + b'a' - 1) as char
                    } else {
                        c.to_ascii_lowercase()
                    };

                    match lower {
                        'c' => {
                            if self.is_generating {
                                self.handle_app_event(AppEvent::StopGeneration).await;
                            } else {
                                self.should_quit = true;
                            }
                            return;
                        }
                        'b' => {
                            self.handle_app_event(AppEvent::ToggleLeftDrawer).await;
                            self.focus = FocusArea::LeftDrawer;
                            return;
                        }
                        'h' => {
                            self.handle_app_event(AppEvent::ToggleRightDrawer).await;
                            self.focus = FocusArea::RightDrawer;
                            return;
                        }
                        'm' => {
                            self.header.toggle_model_dropdown();
                            self.focus = FocusArea::Header;
                            return;
                        }
                        't' => {
                            self.next_theme();
                            return;
                        }
                        'n' => {
                            self.header.toggle_model_dropdown();
                            self.focus = FocusArea::Header;
                            return;
                        }
                        _ => {}
                    }
                }
            }

            // === Tab / Shift+Tab 焦点切换 ===
            if key.code == KeyCode::Tab && !key.modifiers.contains(KeyModifiers::CONTROL) {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    self.cycle_focus(false);
                } else {
                    self.cycle_focus(true);
                }
                return;
            }

            // === Esc ===
            if key.code == KeyCode::Esc && key.modifiers.is_empty() {
                if self.layout.panel != PanelState::None {
                    self.layout.close_drawers();
                } else if self.header.has_dropdown_open() {
                    self.header.close_dropdowns();
                } else {
                    self.focus = FocusArea::Main;
                }
                return;
            }
        }

        // 如果焦点在 Main，按下普通字符/Enter/Backspace 时自动切换到 Input
        if self.focus == FocusArea::Main {
            if let Event::Key(key) = &event {
                if key.kind == KeyEventKind::Press
                    && key.modifiers.is_empty()
                    && matches!(key.code, KeyCode::Char(_) | KeyCode::Enter | KeyCode::Backspace)
                {
                    self.focus = FocusArea::Input;
                }
            }
        }

        // 分发事件到当前焦点组件
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
            AppEvent::SubmitMessage(ref msg) => {
                self.is_generating = true;
                self.chat.add_user_message(msg);
                self.start_chat_stream(msg.clone()).await;
            }
            AppEvent::SseEvent(ref sse_event) => {
                self.chat.handle_sse_event(sse_event);
                if let SseEvent::Done { session_id } = sse_event {
                    self.header.set_session_id(session_id.clone());
                    self.input.set_session_id(Some(session_id.clone()));
                }
            }
            AppEvent::ChatComplete => {
                self.is_generating = false;
            }
            AppEvent::StopGeneration => {
                self.is_generating = false;
            }
            AppEvent::ToggleLeftDrawer => {
                self.layout.toggle_left();
                if self.layout.panel == crate::tui::layout::PanelState::LeftDrawer {
                    self.focus = FocusArea::LeftDrawer;
                    let client = self.client.clone();
                    let tx = self.event_tx.clone();
                    tokio::spawn(async move {
                        if let Ok(tree) = client.get_file_tree(".").await {
                            let _ = tree;
                        }
                    });
                }
            }
            AppEvent::ToggleRightDrawer => {
                self.layout.toggle_right();
                if self.layout.panel == crate::tui::layout::PanelState::RightDrawer {
                    self.focus = FocusArea::RightDrawer;
                }
            }
            AppEvent::CloseDrawers => {
                self.layout.close_drawers();
            }
            AppEvent::SelectModel(ref model) => {
                self.header.set_current_model(model.clone());
            }
            AppEvent::SelectTheme(index) => {
                if index < self.themes.len() {
                    self.theme_index = index;
                    self.theme = self.themes[index].clone();
                }
            }
            AppEvent::SwitchSession(ref id) => {
                let client = self.client.clone();
                let tx = self.event_tx.clone();
                let id = id.clone();
                tokio::spawn(async move {
                    match client.switch_session(&id).await {
                        Ok(_) => {
                            let _ = tx.send(AppEvent::ChatComplete).await;
                        }
                        Err(_) => {}
                    }
                });
            }
            _ => {}
        }

        // Sync StatusBar state
        self.status_bar.set_generating(self.is_generating);
        self.status_bar.set_panel(self.layout.panel);

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
        let session_id = self.header.session_id();

        tokio::spawn(async move {
            let (sse_tx, mut sse_rx) = mpsc::channel(100);

            let forward_handle = {
                let tx = tx.clone();
                tokio::spawn(async move {
                    while let Some(event) = sse_rx.recv().await {
                        let _ = tx.send(AppEvent::SseEvent(event)).await;
                    }
                })
            };

            match client.chat(session_id, message, sse_tx).await {
                Ok(_) => {
                    let _ = forward_handle.await;
                    let _ = tx.send(AppEvent::ChatComplete).await;
                }
                Err(e) => {
                    let _ = forward_handle.await;
                    let _ = tx
                        .send(AppEvent::SseEvent(SseEvent::Error {
                            message: e.to_string(),
                        }))
                        .await;
                    let _ = tx.send(AppEvent::ChatComplete).await;
                }
            }
        });
    }
}
