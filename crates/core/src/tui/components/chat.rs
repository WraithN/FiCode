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

use std::cell::RefCell;

use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
    Frame,
};

use crate::log_debug;
use crate::log_error;
use crate::log_info;
use crate::log_warn;
use crate::server::transport::sse::SseEvent;
use crate::session::message::Part;
use crate::tui::components::part_renderer::PartRendererRegistry;
use crate::tui::components::Component;
use crate::tui::event::AppEvent;
use crate::tui::theme::Theme;

/// 对话回合：包含用户消息和 AI 回复的 Part 列表。
#[derive(Debug, Clone)]
pub struct Turn {
    pub user_message: String,
    pub parts: Vec<Part>,
    pub is_complete: bool,
}

/// 聊天消息结构（保留用于兼容系统消息等旧逻辑）。
#[derive(Debug, Clone)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
}

/// 消息发送者角色。
#[derive(Debug, Clone, PartialEq)]
pub enum MessageRole {
    User,      // 用户
    Assistant, // AI 助手
    System,    // 系统提示
    Error,     // 错误信息
}

/// 聊天组件，负责显示对话历史、处理 SSE 流式消息、渲染生成动画。
pub struct Chat {
    pub turns: Vec<Turn>,                             // 对话回合列表
    messages: Vec<Message>,                       // 保留的系统消息/错误消息（向后兼容）
    pub scroll_offset: usize,                         // 垂直滚动偏移（以行为单位）
    pub is_generating: bool,                          // 是否正在生成回复
    spinner_frame: usize,                         // 当前 spinner 动画帧索引
    pub card_hit_areas: RefCell<Vec<(String, Rect)>>, // 卡片点击区域（card_id -> rect）
    pub renderer_registry: PartRendererRegistry,  // Part 渲染器注册表
}

/// 终端 spinner 动画帧（Braille 点阵字符），每 tick 轮播一帧。
const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

impl Chat {
    pub fn new() -> Self {
        Self {
            turns: Vec::new(),
            messages: Vec::new(),
            scroll_offset: 0,
            is_generating: false,
            spinner_frame: 0,
            card_hit_areas: RefCell::new(Vec::new()),
            renderer_registry: PartRendererRegistry::new(),
        }
    }

    /// 添加一条用户发送的消息，创建新的 Turn。
    pub fn add_user_message(&mut self, content: &str) {
        log_debug!(
            "[Client] Chat add_user_message | turns={} | content_len={}",
            self.turns.len(),
            content.len()
        );
        self.turns.push(Turn {
            user_message: content.to_string(),
            parts: Vec::new(),
            is_complete: false,
        });
    }

    /// 添加一条系统消息（保留向后兼容）。
    pub fn add_system_message(&mut self, content: &str) {
        self.messages.push(Message {
            role: MessageRole::System,
            content: content.to_string(),
        });
    }

    /// 清空所有消息。
    pub fn clear_messages(&mut self) {
        self.turns.clear();
        self.messages.clear();
        self.scroll_offset = 0;
        self.card_hit_areas.borrow_mut().clear();
    }

    /// 定时 tick：若正在生成回复，则推进 spinner 动画帧。
    pub fn on_tick(&mut self) {
        if self.is_generating {
            self.spinner_frame = (self.spinner_frame + 1) % SPINNER_FRAMES.len();
        }
    }

    /// 创建 Thinking 占位 Part（在收到第一个 token 前显示）。
    pub fn create_thinking_card(&mut self) {
        let turn_idx = self.turns.len().saturating_sub(1);
        log_debug!("[Client] Chat create_thinking_card | turn_idx={}", turn_idx);
        if let Some(last_turn) = self.turns.last_mut() {
            last_turn.parts.push(Part::Reasoning {
                thinking: String::new(),
                signature: None,
            });
        }
    }

    /// 处理 SSE 事件：将流式内容追加到当前 Turn 的 Part 列表中。
    pub fn handle_sse_event(&mut self, event: &SseEvent) {
        let Some(last_turn) = self.turns.last_mut() else {
            log_warn!("[Client] Chat handle_sse_event: no turns available");
            return;
        };

        match event {
            SseEvent::Message { content } => {
                log_debug!("[Client] Chat SSE Message | content_len={}", content.len());
                // 追加到最后的 Text Part，或创建新的 Text Part
                if let Some(Part::Text { text }) = last_turn.parts.last_mut() {
                    text.push_str(content);
                } else {
                    // 移除空的 Reasoning（Thinking）占位 Part
                    last_turn.parts.retain(|p| {
                        !(matches!(p, Part::Reasoning { thinking, .. } if thinking.is_empty()))
                    });
                    last_turn.parts.push(Part::Text {
                        text: content.clone(),
                    });
                }
            }
            SseEvent::Part { part } => {
                match part {
                    Part::ToolUse { id, name, .. } => {
                        log_info!("[Client] Chat SSE ToolUse | id={} | name={}", id, name);
                    }
                    Part::ToolResult {
                        tool_call_id,
                        content,
                    } => {
                        log_info!(
                            "[Client] Chat SSE ToolResult | tool_use_id={} | content_len={}",
                            tool_call_id,
                            content.len()
                        );
                    }
                    _ => {}
                }
                last_turn.parts.push(part.clone());
            }
            SseEvent::TaskProgress { plan_id, tasks } => {
                log_debug!(
                    "[Client] Chat SSE TaskProgress | plan_id={} | tasks={}",
                    plan_id,
                    tasks.len()
                );
                let task_count = tasks.len();
                let mut content = String::new();
                for task in tasks {
                    let icon = match task.status {
                        crate::tools::task::TaskStatus::Pending => "⏳",
                        crate::tools::task::TaskStatus::InProgress => "🔵",
                        crate::tools::task::TaskStatus::Completed => "✅",
                        crate::tools::task::TaskStatus::Failed => "❌",
                    };
                    content.push_str(&format!("{} {}\n", icon, task.name));
                }
                last_turn.parts.push(Part::Text {
                    text: format!("Task Plan ({} tasks)\n{}", task_count, content),
                });
            }
            SseEvent::Error { message } => {
                log_error!("[Client] Chat SSE Error | {}", message);
                last_turn.parts.push(Part::Text {
                    text: message.clone(),
                });
            }
            _ => {}
        }
    }

    /// 设置生成状态：开始生成时显示 spinner，结束时重置动画。
    pub fn set_generating(&mut self, generating: bool) {
        log_debug!(
            "[Client] Chat set_generating | {} -> {}",
            self.is_generating,
            generating
        );
        self.is_generating = generating;
        if !generating {
            self.spinner_frame = 0;
            // 标记最后一轮为完成
            if let Some(last) = self.turns.last_mut() {
                last.is_complete = true;
            }
        }
    }

    /// 准备重试指定 Turn：标记为完成并移除错误 Part，返回用户消息。
    pub fn retry_turn(&mut self, turn_index: usize) -> Option<String> {
        let turn = self.turns.get_mut(turn_index)?;
        turn.is_complete = true;
        turn.parts.retain(|p| !matches!(p, Part::ToolError { .. }));
        Some(turn.user_message.clone())
    }

    /// 向上滚动一页。
    fn handle_page_up(&mut self) -> Option<AppEvent> {
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
        Some(AppEvent::ScrollUp)
    }

    /// 计算给定宽度下所有内容的总高度。
    fn total_height(&self, width: u16) -> u16 {
        let mut height = 0u16;
        for turn in &self.turns {
            height += 1; // "You" 前缀
            let content_para = Paragraph::new(turn.user_message.clone()).wrap(Wrap { trim: true });
            height += content_para.line_count(width).max(1) as u16;
            height += 1; // 空行
            for part in &turn.parts {
                if let Some(renderer) = self.renderer_registry.get(part) {
                    height += renderer.height(part, width);
                    height += 1; // Part 间距
                }
            }
        }
        for msg in &self.messages {
            height += 1; // 前缀
            let content_para = Paragraph::new(msg.content.clone()).wrap(Wrap { trim: true });
            height += content_para.line_count(width).max(1) as u16;
            height += 1; // 空行
        }
        if self.is_generating {
            height += 1; // spinner
        }
        height
    }
}

impl Component for Chat {
    fn draw(&self, frame: &mut Frame, area: Rect, theme: &Theme, is_focused: bool) {
        let border_type = if is_focused {
            BorderType::Double
        } else {
            BorderType::Plain
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(border_type)
            .border_style(Style::default().fg(theme.border))
            .style(theme.style_primary());

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let scroll_y = self.scroll_offset as u16;
        let mut current_y = 0u16; // 虚拟 Y 坐标（相对于内容顶部）
        let bottom = inner.y + inner.height;

        // 辅助函数：计算实际渲染 Y 坐标
        let render_y = |cy: u16| -> u16 { inner.y.saturating_add(cy.saturating_sub(scroll_y)) };

        // 辅助函数：判断元素是否可见
        let is_visible =
            |cy: u16, h: u16| -> bool { cy + h > scroll_y && cy < scroll_y + inner.height };

        for turn in &self.turns {
            // 用户消息前缀
            let prefix_height = 1u16;
            if is_visible(current_y, prefix_height) {
                let y = render_y(current_y);
                let para = Paragraph::new(Line::from(vec![
                    Span::styled("● ", theme.style_user()),
                    Span::styled("You", theme.style_user().add_modifier(Modifier::BOLD)),
                ]));
                frame.render_widget(
                    para,
                    Rect {
                        x: inner.x,
                        y,
                        width: inner.width,
                        height: prefix_height.min(bottom.saturating_sub(y)),
                    },
                );
            }
            current_y += prefix_height;

            // 用户消息内容
            let content_para = Paragraph::new(turn.user_message.clone()).wrap(Wrap { trim: true });
            let content_height = content_para.line_count(inner.width).max(1) as u16;
            if is_visible(current_y, content_height) {
                let y = render_y(current_y);
                frame.render_widget(
                    content_para,
                    Rect {
                        x: inner.x,
                        y,
                        width: inner.width,
                        height: content_height.min(bottom.saturating_sub(y)),
                    },
                );
            }
            current_y += content_height;

            // 空行
            current_y += 1;

            // AI Parts
            for part in &turn.parts {
                if let Some(renderer) = self.renderer_registry.get(part) {
                    let part_height = renderer.height(part, inner.width);
                    if is_visible(current_y, part_height) {
                        let y = render_y(current_y);
                        let part_area = Rect {
                            x: inner.x,
                            y,
                            width: inner.width,
                            height: part_height.min(bottom.saturating_sub(y)),
                        };
                        renderer.draw(frame, part_area, part, theme);
                    }
                    current_y += part_height + 1; // +1 for spacing
                }
            }
        }

        // 渲染保留的系统消息/错误消息
        for msg in &self.messages {
            let (prefix, style) = match msg.role {
                MessageRole::User => ("You", theme.style_user().add_modifier(Modifier::BOLD)),
                MessageRole::Assistant => (
                    "◆ FiCodeAgent",
                    theme.style_brand().add_modifier(Modifier::BOLD),
                ),
                MessageRole::System => ("ℹ️ ", Style::default().fg(theme.warning)),
                MessageRole::Error => ("❌ ", Style::default().fg(theme.error)),
            };

            let prefix_height = 1u16;
            if is_visible(current_y, prefix_height) {
                let y = render_y(current_y);
                frame.render_widget(
                    Paragraph::new(Line::from(vec![Span::styled(prefix, style)])),
                    Rect {
                        x: inner.x,
                        y,
                        width: inner.width,
                        height: prefix_height.min(bottom.saturating_sub(y)),
                    },
                );
            }
            current_y += prefix_height;

            let content_para = Paragraph::new(msg.content.clone()).wrap(Wrap { trim: true });
            let content_height = content_para.line_count(inner.width).max(1) as u16;
            if is_visible(current_y, content_height) {
                let y = render_y(current_y);
                frame.render_widget(
                    content_para,
                    Rect {
                        x: inner.x,
                        y,
                        width: inner.width,
                        height: content_height.min(bottom.saturating_sub(y)),
                    },
                );
            }
            current_y += content_height;
            current_y += 1; // 空行
        }

        // Spinner
        if self.is_generating {
            let spinner = SPINNER_FRAMES[self.spinner_frame];
            let spinner_height = 1u16;
            if is_visible(current_y, spinner_height) {
                let y = render_y(current_y);
                let spinner_line = Line::from(vec![
                    Span::styled("◆ ", theme.style_brand()),
                    Span::styled(
                        "FiCodeAgent ",
                        theme.style_brand().add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(spinner, theme.style_brand()),
                ]);
                frame.render_widget(
                    Paragraph::new(spinner_line),
                    Rect {
                        x: inner.x,
                        y,
                        width: inner.width,
                        height: spinner_height.min(bottom.saturating_sub(y)),
                    },
                );
            }
        }

    }

    fn handle_event(&mut self, event: &Event, _focus: bool) -> Option<AppEvent> {
        match event {
            Event::Mouse(mouse) => {
                use crossterm::event::MouseEventKind;
                match mouse.kind {
                    MouseEventKind::ScrollUp => {
                        self.scroll_offset = self.scroll_offset.saturating_sub(3);
                        Some(AppEvent::ScrollUp)
                    }
                    MouseEventKind::ScrollDown => {
                        self.scroll_offset += 3;
                        Some(AppEvent::ScrollDown)
                    }
                    _ => None,
                }
            }
            Event::Key(key) => {
                if key.kind != KeyEventKind::Press {
                    return None;
                }
                match (key.modifiers, key.code) {
                    (KeyModifiers::CONTROL, KeyCode::Up)
                    | (KeyModifiers::NONE, KeyCode::PageUp) => self.handle_page_up(),
                    (KeyModifiers::CONTROL, KeyCode::Down)
                    | (KeyModifiers::NONE, KeyCode::PageDown) => {
                        self.scroll_offset += 1;
                        Some(AppEvent::ScrollDown)
                    }
                    (KeyModifiers::NONE, KeyCode::Char('g')) => {
                        self.turns.iter().rev().find_map(|turn| {
                            turn.parts.iter().find_map(|part| match part {
                                Part::WaveMarker {
                                    git_snapshot: Some(hash),
                                    ..
                                } => Some(AppEvent::BrowseGitSnapshot(hash.clone())),
                                _ => None,
                            })
                        })
                    }
                    (KeyModifiers::NONE, KeyCode::Char('r')) => {
                        self.turns.iter().rev().find_map(|turn| {
                            turn.parts.iter().find_map(|part| match part {
                                Part::WaveMarker {
                                    git_snapshot: Some(snapshot),
                                    step,
                                    ..
                                } => Some(AppEvent::RollbackToWave {
                                    snapshot: snapshot.clone(),
                                    step: *step,
                                }),
                                _ => None,
                            })
                        })
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_user_message_creates_turn() {
        let mut chat = Chat::new();
        chat.add_user_message("hello");
        assert_eq!(chat.turns.len(), 1);
        assert_eq!(chat.turns[0].user_message, "hello");
    }

    #[test]
    fn test_sse_message_creates_text_part() {
        let mut chat = Chat::new();
        chat.add_user_message("hello");
        chat.handle_sse_event(&SseEvent::Message {
            content: "world".to_string(),
        });
        assert_eq!(chat.turns[0].parts.len(), 1);
        assert!(matches!(chat.turns[0].parts[0], Part::Text { .. }));
        if let Part::Text { text } = &chat.turns[0].parts[0] {
            assert_eq!(text, "world");
        }
    }

    #[test]
    fn test_generating_state() {
        let mut chat = Chat::new();
        chat.set_generating(true);
        assert!(chat.is_generating);
        chat.on_tick();
        assert_eq!(chat.spinner_frame, 1);
    }

    #[test]
    fn test_add_system_message() {
        let mut chat = Chat::new();
        chat.add_system_message("System alert");
        assert_eq!(chat.messages.len(), 1);
        assert_eq!(chat.messages[0].role, MessageRole::System);
        assert_eq!(chat.messages[0].content, "System alert");
    }

    #[test]
    fn test_clear_messages() {
        let mut chat = Chat::new();
        chat.add_user_message("hello");
        chat.add_system_message("System alert");
        chat.clear_messages();
        assert!(chat.turns.is_empty());
        assert!(chat.messages.is_empty());
    }

    #[test]
    fn test_tool_use_creates_tool_part() {
        let mut chat = Chat::new();
        chat.add_user_message("run tool");
        chat.handle_sse_event(&SseEvent::Part {
            part: Part::ToolUse {
                id: "tool_1".to_string(),
                name: "bash".to_string(),
                arguments: serde_json::json!({"cmd": "ls"}),
            },
        });
        assert_eq!(chat.turns[0].parts.len(), 1);
        assert!(matches!(chat.turns[0].parts[0], Part::ToolUse { .. }));
    }

    #[test]
    fn test_tool_result_creates_tool_result_part() {
        let mut chat = Chat::new();
        chat.add_user_message("run tool");
        chat.handle_sse_event(&SseEvent::Part {
            part: Part::ToolUse {
                id: "tool_1".to_string(),
                name: "bash".to_string(),
                arguments: serde_json::json!({"cmd": "ls"}),
            },
        });
        chat.handle_sse_event(&SseEvent::Part {
            part: Part::ToolResult {
                tool_call_id: "tool_1".to_string(),
                content: "file.txt".to_string(),
            },
        });
        assert_eq!(chat.turns[0].parts.len(), 2);
        assert!(matches!(chat.turns[0].parts[1], Part::ToolResult { .. }));
    }

    #[test]
    fn test_thinking_placeholder_removed_on_message() {
        let mut chat = Chat::new();
        chat.add_user_message("hello");
        chat.create_thinking_card();
        assert_eq!(chat.turns[0].parts.len(), 1);
        assert!(matches!(chat.turns[0].parts[0], Part::Reasoning { .. }));

        chat.handle_sse_event(&SseEvent::Message {
            content: "world".to_string(),
        });
        assert_eq!(chat.turns[0].parts.len(), 1);
        assert!(matches!(chat.turns[0].parts[0], Part::Text { .. }));
    }
}
