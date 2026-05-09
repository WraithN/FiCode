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

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

/// 消息详情块，用于展示模型的思考过程和工具调用
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DetailBlock {
    Text {
        text: String,
    },
    Reasoning {
        thinking: String,
    },
    ToolUse {
        id: String,
        name: String,
        arguments: String,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        is_error: bool,
    },
}

/// SSE 事件类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SseEvent {
    #[serde(rename = "message")]
    Message { content: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        arguments: Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
    },
    #[serde(rename = "details")]
    MessageDetails { blocks: Vec<DetailBlock> },
    #[serde(rename = "error")]
    Error { message: String },
    #[serde(rename = "done")]
    Done { session_id: String },
}

/// SSE 发送端，供 agent_loop 写入事件
#[derive(Clone)]
pub struct SseSender {
    tx: mpsc::Sender<SseEvent>,
}

impl SseSender {
    pub fn new(tx: mpsc::Sender<SseEvent>) -> Self {
        Self { tx }
    }

    pub async fn send(&self, event: SseEvent) -> Result<(), String> {
        self.tx.send(event).await.map_err(|e| e.to_string())
    }

    /// 同步尝试发送事件（不阻塞，channel 满时返回错误）。
    pub fn try_send(&self, event: SseEvent) -> Result<(), String> {
        self.tx.try_send(event).map_err(|e| e.to_string())
    }
}

/// 创建 SSE 流对 (sender, stream)
pub fn create_sse_channel(buffer: usize) -> (SseSender, ReceiverStream<SseEvent>) {
    let (tx, rx) = mpsc::channel::<SseEvent>(buffer);
    (SseSender::new(tx), ReceiverStream::new(rx))
}

/// 将 SseEvent 序列化为 SSE data 行
pub fn format_sse_event(event: SseEvent) -> String {
    let data = serde_json::to_string(&event).unwrap_or_default();
    format!("data: {}\n\n", data)
}
