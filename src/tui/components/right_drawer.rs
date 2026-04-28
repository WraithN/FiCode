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
