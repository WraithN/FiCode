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
            SseEvent::Message { content } => {
                if let Some(last) = self.messages.last_mut() {
                    if last.role == MessageRole::Assistant {
                        last.content.push_str(content);
                    } else {
                        self.messages.push(Message {
                            role: MessageRole::Assistant,
                            content: content.clone(),
                        });
                    }
                } else {
                    self.messages.push(Message {
                        role: MessageRole::Assistant,
                        content: content.clone(),
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
                MessageRole::Assistant => {
                    ("◆ AI", theme.style_brand().add_modifier(Modifier::BOLD))
                }
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
                (KeyModifiers::CONTROL, KeyCode::Down)
                | (KeyModifiers::NONE, KeyCode::PageDown) => {
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
    fn test_sse_message_appends() {
        let mut chat = Chat::new();
        chat.handle_sse_event(&SseEvent::Message {
            content: "Hello".to_string(),
        });
        chat.handle_sse_event(&SseEvent::Message {
            content: " world".to_string(),
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
