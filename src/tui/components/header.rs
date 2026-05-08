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
use crate::tui::event::{AppEvent, ModelItem, ProviderItem};
use crate::tui::theme::Theme;

/// 顶部状态栏的当前状态。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HeaderStatus {
    Ready,      // 空闲
    Generating, // 正在生成
    Streaming,  // 正在流式传输
}

/// 模型菜单状态机。
#[derive(Debug, Clone)]
enum MenuState {
    Closed,
    ProviderList,
    ModelList { provider_idx: usize },
}

/// 顶部标题栏组件，展示 Logo、当前模型、运行状态，以及模型两级子菜单。
pub struct Header {
    current_model: String,
    session_id: Option<String>,
    menu_state: MenuState,
    providers: Vec<ProviderItem>,
    provider_selected: usize,
    model_selected: Vec<usize>, // 每个 provider 对应的选中模型索引
    status: HeaderStatus,
}

impl Header {
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

    pub fn set_current_model(&mut self, model: String) {
        self.current_model = model;
    }

    pub fn set_session_id(&mut self, id: String) {
        self.session_id = Some(id);
    }

    pub fn session_id(&self) -> Option<String> {
        self.session_id.clone()
    }

    /// 切换模型菜单（打开时默认进入 Provider 列表）。
    pub fn toggle_model_dropdown(&mut self) {
        match self.menu_state {
            MenuState::Closed => {
                self.menu_state = MenuState::ProviderList;
                self.provider_selected = 0;
            }
            _ => self.menu_state = MenuState::Closed,
        }
    }

    pub fn toggle_theme_dropdown(&mut self) {
        // 主题下拉保持原行为：关闭模型菜单
        self.menu_state = MenuState::Closed;
    }

    pub fn close_dropdowns(&mut self) {
        self.menu_state = MenuState::Closed;
    }

    pub fn has_dropdown_open(&self) -> bool {
        !matches!(self.menu_state, MenuState::Closed)
    }

    pub fn on_tick(&mut self) {}

    pub fn set_status(&mut self, status: HeaderStatus) {
        self.status = status;
    }

    /// 设置从后端加载的模型列表。
    pub fn set_providers(&mut self, providers: Vec<ProviderItem>) {
        self.model_selected = vec![0; providers.len()];
        self.providers = providers;
        // 保持当前选中在有效范围内
        if self.provider_selected >= self.providers.len() && !self.providers.is_empty() {
            self.provider_selected = self.providers.len() - 1;
        }
    }

    /// 返回当前是否需要从后端加载模型列表。
    pub fn needs_load_models(&self) -> bool {
        matches!(self.menu_state, MenuState::ProviderList) && self.providers.is_empty()
    }

    /// 按 key 查找 provider。
    pub fn get_provider(&self, key: &str) -> Option<&ProviderItem> {
        self.providers.iter().find(|p| p.key == key)
    }

    /// 获取所有 provider 的引用。
    pub fn providers(&self) -> &[ProviderItem] {
        &self.providers
    }
}

impl Component for Header {
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

        match &self.menu_state {
            MenuState::ProviderList => self.draw_provider_list(frame, area, theme),
            MenuState::ModelList { provider_idx } => self.draw_model_list(frame, area, theme, *provider_idx),
            MenuState::Closed => {}
        }
    }

    fn handle_event(&mut self, event: &Event, _focus: bool) -> Option<AppEvent> {
        if let Event::Key(key) = event {
            if key.kind != KeyEventKind::Press {
                return None;
            }

            match &mut self.menu_state {
                MenuState::ProviderList => self.handle_provider_list_event(key.code),
                MenuState::ModelList { provider_idx } => {
                    let idx = *provider_idx;
                    self.handle_model_list_event(key.code, idx)
                }
                MenuState::Closed => None,
            }
        } else {
            None
        }
    }
}

impl Header {
    fn draw_provider_list(&self, frame: &mut Frame, header_area: Rect, theme: &Theme) {
        let items: Vec<Line> = self
            .providers
            .iter()
            .enumerate()
            .map(|(i, provider)| {
                let prefix = if i == self.provider_selected { "▶ " } else { "  " };
                let style = if i == self.provider_selected {
                    theme.style_selection()
                } else {
                    theme.style_primary()
                };
                Line::styled(format!("{}{}", prefix, provider.name), style)
            })
            .collect();

        let height = items.len().clamp(3, 8) as u16 + 2;
        let width = 34u16;
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

    fn draw_model_list(&self, frame: &mut Frame, header_area: Rect, theme: &Theme, provider_idx: usize) {
        let provider = match self.providers.get(provider_idx) {
            Some(p) => p,
            None => return,
        };

        let mut items = vec![Line::styled(
            format!("◀ {}", provider.name),
            Style::default().fg(theme.border).add_modifier(Modifier::BOLD),
        )];

        let selected = self.model_selected.get(provider_idx).copied().unwrap_or(0);
        for (i, model) in provider.models.iter().enumerate() {
            let prefix = if i == selected { "● " } else { "  " };
            let style = if i == selected {
                theme.style_selection()
            } else {
                theme.style_primary()
            };
            items.push(Line::styled(format!("{}{}", prefix, model.name), style));
        }

        let height = items.len().clamp(3, 10) as u16 + 2;
        let width = 38u16;
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

    fn handle_provider_list_event(&mut self, code: KeyCode) -> Option<AppEvent> {
        let max = self.providers.len().saturating_sub(1);
        match code {
            KeyCode::Up => {
                if self.provider_selected > 0 {
                    self.provider_selected -= 1;
                }
                Some(AppEvent::InputChanged(String::new()))
            }
            KeyCode::Down => {
                if self.provider_selected < max {
                    self.provider_selected += 1;
                }
                Some(AppEvent::InputChanged(String::new()))
            }
            KeyCode::Enter => {
                if !self.providers.is_empty() {
                    let idx = self.provider_selected;
                    self.menu_state = MenuState::ModelList { provider_idx: idx };
                }
                Some(AppEvent::InputChanged(String::new()))
            }
            KeyCode::Esc => {
                self.menu_state = MenuState::Closed;
                None
            }
            _ => None,
        }
    }

    fn handle_model_list_event(&mut self, code: KeyCode, provider_idx: usize) -> Option<AppEvent> {
        let provider = match self.providers.get(provider_idx) {
            Some(p) => p,
            None => {
                self.menu_state = MenuState::ProviderList;
                return None;
            }
        };
        let max = provider.models.len().saturating_sub(1);
        let selected = self.model_selected.get_mut(provider_idx)?;

        match code {
            KeyCode::Up => {
                if *selected > 0 {
                    *selected -= 1;
                }
                Some(AppEvent::InputChanged(String::new()))
            }
            KeyCode::Down => {
                if *selected < max {
                    *selected += 1;
                }
                Some(AppEvent::InputChanged(String::new()))
            }
            KeyCode::Enter => {
                let model = provider.models.get(*selected)?;
                let provider_key = provider.key.clone();
                let model_key = model.key.clone();
                // 预设 provider（非 custom）默认 api_key 为空，弹出模态框让用户输入
                let needs_key = provider.provider_type != "custom" && provider_key != "custom";
                self.menu_state = MenuState::Closed;
                if needs_key {
                    Some(AppEvent::SelectModelItem {
                        provider: provider_key,
                        model: model_key,
                    })
                } else {
                    Some(AppEvent::SwitchModel {
                        provider: provider_key,
                        model: model_key,
                        api_key: None,
                    })
                }
            }
            KeyCode::Esc => {
                self.menu_state = MenuState::ProviderList;
                Some(AppEvent::InputChanged(String::new()))
            }
            _ => None,
        }
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
    fn test_menu_toggle() {
        let mut header = Header::new();
        assert!(!header.has_dropdown_open());
        header.toggle_model_dropdown();
        assert!(header.has_dropdown_open());
        header.toggle_model_dropdown();
        assert!(!header.has_dropdown_open());
    }
}
