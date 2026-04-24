use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

/// SSE 事件类型
#[derive(Debug, Clone, Serialize)]
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
