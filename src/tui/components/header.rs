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

use crossterm::event::{Event, KeyCode, KeyEventKind};
use ratatui::{
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::tui::components::Component;
use crate::tui::event::AppEvent;
use crate::tui::theme::Theme;

/// 模型信息结构。
#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub name: String,
    pub capabilities: Vec<String>,
}

/// 顶部状态栏的当前状态。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HeaderStatus {
    Ready,      // 空闲
    Generating, // 正在生成
    Streaming,  // 正在流式传输
}

/// 顶部标题栏组件，展示 Logo、当前模型、运行状态，以及模型下拉菜单。
pub struct Header {
    current_model: String,
    session_id: Option<String>,
    model_dropdown_open: bool,   // 模型下拉菜单是否展开
    theme_dropdown_open: bool,   // 主题下拉菜单是否展开（预留）
    dropdown_selected: usize,    // 下拉菜单当前选中项
    models: Vec<ModelInfo>,      // 可选模型列表
    status: HeaderStatus,
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

    pub fn close_dropdowns(&mut self) {
        self.model_dropdown_open = false;
        self.theme_dropdown_open = false;
    }

    pub fn has_dropdown_open(&self) -> bool {
        self.model_dropdown_open || self.theme_dropdown_open
    }

    pub fn on_tick(&mut self) {}

    /// 更新状态栏显示的状态。
    pub fn set_status(&mut self, status: HeaderStatus) {
        self.status = status;
    }
}

impl Component for Header {
    /// 渲染标题栏：左侧显示 Logo、模型名、状态指示器；若下拉菜单打开则叠加渲染菜单。
    fn draw(&self, frame: &mut Frame, area: Rect, theme: &Theme, _is_focused: bool) {
        let block = Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(theme.border))
            .style(theme.header_style());

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let logo = Span::styled("FiCode", theme.style_brand().add_modifier(Modifier::BOLD));

        let model_text = format!("▼ {}", self.current_model);
        let model = Span::styled(model_text, theme.style_primary());

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

        if self.model_dropdown_open {
            self.draw_model_dropdown(frame, area, theme);
        }
    }

    /// 处理标题栏事件：当下拉菜单打开时，拦截方向键、Enter、Esc 进行菜单导航。
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
    /// 渲染模型下拉菜单：在标题栏下方弹出，显示可选模型列表与当前选中高亮。
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

        let area = ratatui::layout::Rect::new(x, y, width, height);
        frame.render_widget(Clear, area);

        let paragraph = Paragraph::new(items).block(
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
