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

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::server::transport::sse::TaskProgressItem;
use crate::tui::event::CardAction;
use crate::tui::theme::Theme;

/// 卡片数据结构，表示聊天界面中的一个结构化信息块。
#[derive(Debug, Clone)]
pub struct Card {
    pub id: String,
    pub kind: CardKind,
    pub title: String,
    pub content: String,
    pub full_content: Option<String>,
    pub right_content: Option<String>,
    pub state: CardState,
}

/// 卡片类型枚举。
#[derive(Debug, Clone)]
pub enum CardKind {
    Thinking,
    ToolUse { name: String },
    ToolResult,
    WriteFile { path: String },
    TodoList {
        plan_id: String,
        tasks: Vec<TaskProgressItem>,
    },
    Summary,
    Error,
}

/// 卡片状态枚举。
#[derive(Debug, Clone, PartialEq)]
pub enum CardState {
    Animating,
    Collapsed,
    Expanded,
    Completed,
}

/// 卡片渲染组件。
pub struct CardWidget<'a> {
    card: &'a Card,
}

impl<'a> CardWidget<'a> {
    pub fn new(card: &'a Card) -> Self {
        Self { card }
    }

    /// 计算卡片在给定宽度下的渲染高度。
    pub fn calculate_height(&self, width: u16) -> u16 {
        let title_height = 1;
        let content_lines = self.card.content.lines().count() as u16;
        let footer_height = if self.show_footer() { 1 } else { 0 };
        let padding = 2; // top/bottom border
        title_height + content_lines.min(20) + footer_height + padding
    }

    fn show_footer(&self) -> bool {
        (matches!(self.card.state, CardState::Collapsed | CardState::Expanded)
            && self.card.full_content.is_some())
            || matches!(self.card.kind, CardKind::Error)
    }

    /// 在指定区域绘制卡片。
    pub fn draw(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border))
            .style(theme.style_primary());

        let inner = block.inner(area);
        frame.render_widget(block, area);

        // Split inner area: title (1) + content (rest-1) + footer (1)
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(1),
                Constraint::Length(if self.show_footer() { 1 } else { 0 }),
            ])
            .split(inner);

        // Title bar
        let icon = match &self.card.kind {
            CardKind::Thinking => "🧠",
            CardKind::ToolUse { .. } => "🔧",
            CardKind::ToolResult => "📤",
            CardKind::WriteFile { .. } => "📝",
            CardKind::TodoList { .. } => "📋",
            CardKind::Summary => "◆ AI",
            CardKind::Error => "❌",
        };
        let title_line = Line::from(vec![
            Span::styled(format!("{} ", icon), theme.style_brand()),
            Span::styled(
                &self.card.title,
                theme.style_brand().add_modifier(Modifier::BOLD),
            ),
        ]);
        frame.render_widget(Paragraph::new(title_line), chunks[0]);

        // Content area (with optional right panel)
        if self.card.right_content.is_some()
            && !matches!(self.card.kind, CardKind::TodoList { .. })
        {
            let h_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
                .split(chunks[1]);

            let content_text = Text::from(self.card.content.clone());
            frame.render_widget(
                Paragraph::new(content_text).wrap(Wrap { trim: true }),
                h_chunks[0],
            );

            let right_text = Text::from(self.card.right_content.clone().unwrap());
            frame.render_widget(
                Paragraph::new(right_text).wrap(Wrap { trim: true }),
                h_chunks[1],
            );
        } else {
            let content_text = Text::from(self.card.content.clone());
            frame.render_widget(
                Paragraph::new(content_text).wrap(Wrap { trim: true }),
                chunks[1],
            );
        }

        // Footer
        if self.show_footer() {
            let footer_text = match &self.card.kind {
                CardKind::Error => "[Retry]",
                _ => {
                    if self.card.state == CardState::Expanded {
                        "−Collapse"
                    } else {
                        "+Expand"
                    }
                }
            };
            let footer_line = Line::from(Span::styled(
                footer_text,
                Style::default()
                    .fg(theme.brand)
                    .add_modifier(Modifier::UNDERLINED),
            ));
            let footer_para = Paragraph::new(footer_line).alignment(Alignment::Right);
            frame.render_widget(footer_para, chunks[2]);
        }
    }

    /// 处理鼠标点击事件，返回对应的 CardAction。
    pub fn handle_click(&self, x: u16, y: u16, rect: Rect) -> Option<CardAction> {
        if !self.show_footer() {
            return None;
        }
        // Calculate footer area
        let footer_y = rect.y + rect.height - 2; // account for border
        let footer_area = Rect {
            x: rect.x + 2,
            y: footer_y,
            width: rect.width - 4,
            height: 1,
        };

        if y == footer_y && x >= footer_area.x && x < footer_area.x + footer_area.width {
            match &self.card.kind {
                CardKind::Error => Some(CardAction::Retry(self.card.id.clone())),
                _ => {
                    if self.card.state == CardState::Expanded {
                        Some(CardAction::Collapse(self.card.id.clone()))
                    } else {
                        Some(CardAction::Expand(self.card.id.clone()))
                    }
                }
            }
        } else {
            None
        }
    }
}
