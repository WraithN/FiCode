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
