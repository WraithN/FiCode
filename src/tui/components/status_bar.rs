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

use crossterm::event::Event;
use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::tui::components::Component;
use crate::tui::event::AppEvent;
use crate::tui::layout::PanelState;
use crate::tui::theme::Theme;

pub struct StatusBar {
    is_generating: bool,
    panel: PanelState,
}

impl StatusBar {
    pub fn new() -> Self {
        Self {
            is_generating: false,
            panel: PanelState::None,
        }
    }

    pub fn set_generating(&mut self, generating: bool) {
        self.is_generating = generating;
    }

    pub fn set_panel(&mut self, panel: PanelState) {
        self.panel = panel;
    }
}

impl Component for StatusBar {
    fn draw(&self, frame: &mut Frame, area: Rect, theme: &Theme, _is_focused: bool) {
        let mut spans = vec![];

        let files_label = match self.panel {
            PanelState::LeftDrawer => "[Ctrl+B] Hide",
            _ => "[Ctrl+B] Files",
        };
        spans.push(Span::styled(files_label, theme.style_muted()));
        spans.push(Span::raw("  "));

        let history_label = match self.panel {
            PanelState::RightDrawer => "[Ctrl+H] Hide",
            _ => "[Ctrl+H] History",
        };
        spans.push(Span::styled(history_label, theme.style_muted()));
        spans.push(Span::raw("  "));

        spans.push(Span::styled("[Ctrl+M] Model", theme.style_muted()));
        spans.push(Span::raw("  "));

        spans.push(Span::styled("[Ctrl+T] Theme", theme.style_muted()));
        spans.push(Span::raw("  "));

        spans.push(Span::styled("[Ctrl+N] New", theme.style_muted()));

        if self.is_generating {
            spans.push(Span::raw("  "));
            spans.push(Span::styled(
                "[Ctrl+C] Stop",
                Style::default().fg(theme.error),
            ));
        }

        let line = Line::from(spans);
        let paragraph = Paragraph::new(line).style(theme.status_bar_style());
        frame.render_widget(paragraph, area);
    }

    fn handle_event(&mut self, _event: &Event, _focus: bool) -> Option<AppEvent> {
        None
    }

    fn is_focusable(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_bar_state() {
        let mut bar = StatusBar::new();
        assert!(!bar.is_generating);
        bar.set_generating(true);
        assert!(bar.is_generating);
    }
}
