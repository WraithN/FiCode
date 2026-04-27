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
