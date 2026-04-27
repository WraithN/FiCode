use crossterm::event::Event;
use ratatui::{layout::Rect, Frame};

use crate::tui::event::AppEvent;
use crate::tui::theme::Theme;

pub mod chat;
pub mod header;
pub mod input;
pub mod left_drawer;
pub mod right_drawer;
pub mod status_bar;

pub trait Component {
    fn draw(&self, frame: &mut Frame, area: Rect, theme: &Theme);
    fn handle_event(&mut self, event: &Event, focus: bool) -> Option<AppEvent>;
    fn update(&mut self, _event: &AppEvent) {}
    fn is_focusable(&self) -> bool {
        true
    }
}
