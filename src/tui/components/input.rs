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

use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
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
    session_id: Option<String>,
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
            session_id: None,
        }
    }

    pub fn set_session_id(&mut self, id: Option<String>) {
        self.session_id = id;
    }

    pub fn visible_lines(&self) -> u16 {
        2  // 固定 2 行高度
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
    fn draw(&self, frame: &mut Frame, area: Rect, theme: &Theme, is_focused: bool) {
        // 在输入框上方显示会话 ID
        let mut y_offset = 0u16;
        if let Some(ref id) = self.session_id {
            let session_label = format!("--[Session: {}]---", id);
            let label_rect = Rect::new(area.x, area.y, area.width, 1);
            let label = Paragraph::new(session_label)
                .style(theme.style_muted());
            frame.render_widget(label, label_rect);
            y_offset = 1;
        }

        let placeholder = if self.content.is_empty() {
            "Type your message, or paste code..."
        } else {
            ""
        };

        let border_type = if is_focused {
            ratatui::widgets::BorderType::Double
        } else {
            ratatui::widgets::BorderType::Plain
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(border_type)
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

        // 设置闪烁光标位置（支持多行）
        let text_before_cursor = &self.content[..self.cursor_position];
        let lines: Vec<&str> = text_before_cursor.split('\n').collect();
        let cursor_row = lines.len().saturating_sub(1) as u16;
        let cursor_col = lines.last().unwrap_or(&"").chars().count() as u16;
        let cursor_x = inner.x + cursor_col;
        let cursor_y = inner.y + cursor_row;
        frame.set_cursor_position((cursor_x, cursor_y));

        if self.dropdown_visible && !self.dropdown_items.is_empty() {
            self.draw_dropdown(frame, area, theme);
        }
    }

    fn handle_event(&mut self, event: &Event, _focus: bool) -> Option<AppEvent> {
        if let Event::Key(key) = event {
            if key.kind != KeyEventKind::Press {
                return None;
            }

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
