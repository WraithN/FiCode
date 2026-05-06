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

/// 底部状态栏组件，始终显示快捷键提示与当前生成状态。
///
/// 该组件不可聚焦，仅作为信息展示。
pub struct StatusBar {
    is_generating: bool, // 是否正在生成回复（控制是否显示 Stop 提示）
    panel: PanelState,   // 当前面板状态（控制 Files/History 按钮显示文字）
}

impl StatusBar {
    pub fn new() -> Self {
        Self {
            is_generating: false,
            panel: PanelState::None,
        }
    }

    /// 更新生成状态。
    pub fn set_generating(&mut self, generating: bool) {
        self.is_generating = generating;
    }

    /// 更新面板状态。
    pub fn set_panel(&mut self, panel: PanelState) {
        self.panel = panel;
    }
}

impl Component for StatusBar {
    /// 渲染状态栏：左侧显示常用快捷键（Files、History、Model、Theme、New），
    /// 若正在生成则追加红色的 `[Ctrl+C] Stop` 提示。
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

    /// 状态栏不处理任何事件。
    fn handle_event(&mut self, _event: &Event, _focus: bool) -> Option<AppEvent> {
        None
    }

    /// 状态栏不可聚焦。
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
