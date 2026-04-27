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
    pub fn set_generating(&mut self, _generating: bool) {}
}

impl Component for Chat {
    fn draw(&self, _frame: &mut Frame, _area: Rect, _theme: &Theme) {}
    fn handle_event(&mut self, _event: &Event, _focus: bool) -> Option<AppEvent> {
        None
    }
}
