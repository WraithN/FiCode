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
    pub fn session_id(&self) -> Option<String> {
        None
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
