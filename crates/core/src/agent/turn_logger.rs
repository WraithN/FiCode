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

// =============================================================================
// turn_logger 模块：Turn 级完整对话日志的异步持久化
// =============================================================================
// 本模块通过 `tokio::sync::mpsc` 将 `TurnLogEntry` 异步发送给后台写入任务，
// 避免阻塞 Agent 循环。日志以 JSON Lines 格式追加到 `~/.config/fi-code/logs/turns.jsonl`。

use serde::Serialize;
use std::sync::OnceLock;
use tokio::sync::mpsc;

use crate::provider::base_client::TokenUsage;
use crate::session::message::{Message, Part};

/// 工具执行结果日志项。
#[derive(Debug, Clone, Serialize)]
pub struct ToolResultLog {
    pub tool_call_id: String,
    pub name: String,
    pub arguments: serde_json::Value,
    pub content: String,
    pub duration_ms: u64,
    pub is_error: bool,
}

/// 单轮对话完整日志项。
#[derive(Debug, Clone, Serialize)]
pub struct TurnLogEntry {
    /// ISO 8601 格式时间戳
    pub timestamp: String,
    /// 当前会话 ID
    pub session_id: String,
    /// 本轮序号（从 1 开始）
    pub turn_index: usize,
    /// LLM 停止原因
    pub finish_reason: Option<String>,
    /// 本轮 Token 使用量
    pub token_usage: TokenUsage,
    /// LLM 流式输出的所有 Part
    pub content_blocks: Vec<Part>,
    /// 工具执行结果
    pub tool_results: Vec<ToolResultLog>,
    /// 本轮结束时的 messages 数组快照
    pub messages_snapshot: Vec<Message>,
    /// WaveMarker 元信息
    pub wave_marker: Option<Part>,
    /// Agent 状态迁移原因
    pub transition_reason: Option<String>,
    /// 若本轮执行出错，记录错误信息
    pub error: Option<String>,
}

/// 后台写入通道句柄。
pub struct TurnLogger {
    tx: mpsc::Sender<TurnLogEntry>,
}

impl TurnLogger {
    /// 获取全局单例。
    pub fn global() -> &'static TurnLogger {
        static INSTANCE: OnceLock<TurnLogger> = OnceLock::new();
        INSTANCE.get_or_init(|| {
            let (tx, rx) = mpsc::channel::<TurnLogEntry>(1024);
            tokio::spawn(async move {
                Self::run_writer(rx).await;
            });
            TurnLogger { tx }
        })
    }

    /// 发送日志条目（非阻塞，缓冲区满时丢弃）。
    pub fn log_turn(&self, entry: TurnLogEntry) {
        let _ = self.tx.try_send(entry);
    }

    /// 后台写入任务：持续接收条目并追加到 JSONL 文件。
    async fn run_writer(mut rx: mpsc::Receiver<TurnLogEntry>) {
        use tokio::io::AsyncWriteExt;

        let Some(proj_dirs) = directories::ProjectDirs::from("", "", "fi-code") else {
            return;
        };
        let logs_dir = proj_dirs.config_dir().join("logs");
        let _ = tokio::fs::create_dir_all(&logs_dir).await;
        let path = logs_dir.join("turns.jsonl");
        let Ok(mut file) = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await
        else {
            return;
        };

        while let Some(entry) = rx.recv().await {
            let line = match serde_json::to_string(&entry) {
                Ok(s) => s,
                Err(_) => continue,
            };
            let _ = file.write_all(line.as_bytes()).await;
            let _ = file.write_all(b"\n").await;
        }
    }
}

/// 将工具执行结果（Part 数组）转换为可序列化的日志结构。
///
/// `content_blocks` 中应包含对应的 `ToolUse`，用于提取工具名称和参数。
pub fn build_tool_result_logs(
    content_blocks: &[Part],
    tool_results: &[Part],
) -> Vec<ToolResultLog> {
    let mut logs = Vec::with_capacity(tool_results.len());

    for result in tool_results {
        let (tool_call_id, content, duration_ms, is_error) = match result {
            Part::ToolResult {
                tool_call_id,
                content,
                duration_ms,
                ..
            } => (tool_call_id, content, duration_ms.unwrap_or(0), false),
            Part::ToolError {
                tool_call_id,
                content,
                ..
            } => (tool_call_id, content, 0, true),
            _ => continue,
        };

        // 从 content_blocks 中查找匹配的 ToolUse 以获取名称和参数
        let (name, arguments) = content_blocks
            .iter()
            .find_map(|p| {
                if let Part::ToolUse {
                    id,
                    name,
                    arguments,
                } = p
                {
                    if id == tool_call_id {
                        Some((name.clone(), arguments.clone()))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .unwrap_or_else(|| ("unknown".to_string(), serde_json::Value::Null));

        logs.push(ToolResultLog {
            tool_call_id: tool_call_id.clone(),
            name,
            arguments,
            content: content.clone(),
            duration_ms,
            is_error,
        });
    }

    logs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_turn_log_entry_serialization() {
        let entry = TurnLogEntry {
            timestamp: "2026-05-15T10:00:00+08:00".to_string(),
            session_id: "test-session".to_string(),
            turn_index: 1,
            finish_reason: Some("stop".to_string()),
            token_usage: TokenUsage::default(),
            content_blocks: vec![],
            tool_results: vec![],
            messages_snapshot: vec![],
            wave_marker: None,
            transition_reason: None,
            error: None,
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("test-session"));
        assert!(json.contains("turn_index"));
    }

    #[test]
    fn test_tool_result_log_serialization() {
        let tr = ToolResultLog {
            tool_call_id: "call_1".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({"command": "echo hello"}),
            content: "hello".to_string(),
            duration_ms: 100,
            is_error: false,
        };
        let json = serde_json::to_string(&tr).unwrap();
        assert!(json.contains("bash"));
        assert!(json.contains("hello"));
    }

    #[test]
    fn test_build_tool_result_logs() {
        let content_blocks = vec![Part::ToolUse {
            id: "call_1".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({"command": "ls"}),
        }];

        let tool_results = vec![Part::ToolResult {
            tool_call_id: "call_1".to_string(),
            content: "file.txt".to_string(),
            duration_ms: Some(42),
            metadata: None,
            for_context_only: false,
        }];

        let logs = build_tool_result_logs(&content_blocks, &tool_results);
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].name, "bash");
        assert_eq!(logs[0].duration_ms, 42);
        assert!(!logs[0].is_error);
    }

    #[test]
    fn test_build_tool_result_logs_with_error() {
        let content_blocks = vec![Part::ToolUse {
            id: "call_2".to_string(),
            name: "read".to_string(),
            arguments: serde_json::json!({"path": "/missing"}),
        }];

        let tool_results = vec![Part::ToolError {
            tool_call_id: "call_2".to_string(),
            content: "failed".to_string(),
            error_message: "not found".to_string(),
            for_context_only: false,
        }];

        let logs = build_tool_result_logs(&content_blocks, &tool_results);
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].name, "read");
        assert!(logs[0].is_error);
        assert_eq!(logs[0].duration_ms, 0);
    }
}
