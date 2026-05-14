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
use std::collections::VecDeque;
use tokio::sync::broadcast;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub module: String,
    pub message: String,
}

pub struct LogStore {
    buffer: VecDeque<LogEntry>,
    capacity: usize,
}

impl LogStore {
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub fn push(&mut self, entry: LogEntry) {
        if self.buffer.len() >= self.capacity {
            self.buffer.pop_front();
        }
        self.buffer.push_back(entry);
    }

    pub fn recent(&self, limit: usize) -> Vec<LogEntry> {
        self.buffer
            .iter()
            .rev()
            .take(limit)
            .rev()
            .cloned()
            .collect()
    }
}

/// 日志文件写入器，在后台任务中异步将日志写入本地文件。
///
/// 按模块路径区分：
/// - 模块名包含 `tui` 或 `fi_code_tui` → `tui.log`
/// - 其他 → `agent.log`
pub struct LogFileWriter {
    agent_file: tokio::fs::File,
    tui_file: tokio::fs::File,
}

impl LogFileWriter {
    async fn new(logs_dir: std::path::PathBuf) -> anyhow::Result<Self> {
        tokio::fs::create_dir_all(&logs_dir).await?;
        let agent_path = logs_dir.join("agent.log");
        let tui_path = logs_dir.join("tui.log");
        let agent_file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&agent_path)
            .await?;
        let tui_file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&tui_path)
            .await?;
        Ok(Self {
            agent_file,
            tui_file,
        })
    }

    /// 格式化日志条目为单行文本。
    fn format_entry(entry: &LogEntry) -> String {
        format!(
            "{} [{}] [{}] {}\n",
            entry.timestamp, entry.level, entry.module, entry.message
        )
    }

    /// 判断日志是否属于 TUI 模块。
    fn is_tui_module(module: &str) -> bool {
        module.contains("tui") || module.contains("fi_code_tui")
    }

    /// 持续接收日志条目并写入对应文件。
    async fn run(mut self, mut rx: tokio::sync::mpsc::Receiver<LogEntry>) {
        use tokio::io::AsyncWriteExt;
        while let Some(entry) = rx.recv().await {
            let line = Self::format_entry(&entry);
            let file = if Self::is_tui_module(&entry.module) {
                &mut self.tui_file
            } else {
                &mut self.agent_file
            };
            // 异步写入，失败时静默丢弃（避免阻塞或 panic）
            let _ = file.write_all(line.as_bytes()).await;
        }
    }
}

pub struct LogBroadcaster {
    tx: broadcast::Sender<LogEntry>,
    store: std::sync::Mutex<LogStore>,
    /// 文件写入通道，若当前不在 tokio 运行时中则为 None（如单元测试）
    file_tx: Option<tokio::sync::mpsc::Sender<LogEntry>>,
}

impl LogBroadcaster {
    pub fn new(capacity: usize) -> Self {
        let (tx, _rx) = broadcast::channel(256);

        // 尝试启动异步文件写入任务（仅在 tokio 运行时中生效）
        let file_tx = if let Ok(handle) = tokio::runtime::Handle::try_current() {
            let (file_tx, file_rx) = tokio::sync::mpsc::channel(1024);
            handle.spawn(async move {
                if let Some(proj_dirs) = directories::ProjectDirs::from("", "", "fi-code") {
                    let logs_dir = proj_dirs.config_dir().join("logs");
                    if let Ok(writer) = LogFileWriter::new(logs_dir).await {
                        writer.run(file_rx).await;
                    }
                }
            });
            Some(file_tx)
        } else {
            None
        };

        Self {
            tx,
            store: std::sync::Mutex::new(LogStore::new(capacity)),
            file_tx,
        }
    }

    /// 同步方法，供日志宏在非 async 上下文中调用。
    ///
    /// 日志会同时进入：
    /// 1. 内存环形缓冲区（供 TUI/Server 实时查看）
    /// 2. 广播通道（供 SSE 流推送）
    /// 3. 文件写入通道（异步写入本地文件，不阻塞）
    pub fn send(&self, level: &str, module: &str, message: String) {
        let entry = LogEntry {
            timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
            level: level.to_string(),
            module: module.to_string(),
            message,
        };
        if let Ok(mut store) = self.store.lock() {
            store.push(entry.clone());
        }
        if let Some(ref tx) = self.file_tx {
            // try_send 不会阻塞，若通道满则丢弃旧日志
            let _ = tx.try_send(entry.clone());
        }
        let _ = self.tx.send(entry);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<LogEntry> {
        self.tx.subscribe()
    }

    pub fn recent(&self, limit: usize) -> Vec<LogEntry> {
        if let Ok(store) = self.store.lock() {
            store.recent(limit)
        } else {
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_store_capacity() {
        let mut store = LogStore::new(3);
        store.push(LogEntry {
            timestamp: "00:00:00".into(),
            level: "INFO".into(),
            module: "a".into(),
            message: "1".into(),
        });
        store.push(LogEntry {
            timestamp: "00:00:01".into(),
            level: "INFO".into(),
            module: "a".into(),
            message: "2".into(),
        });
        store.push(LogEntry {
            timestamp: "00:00:02".into(),
            level: "INFO".into(),
            module: "a".into(),
            message: "3".into(),
        });
        store.push(LogEntry {
            timestamp: "00:00:03".into(),
            level: "INFO".into(),
            module: "a".into(),
            message: "4".into(),
        });
        let recent = store.recent(10);
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].message, "2");
        assert_eq!(recent[2].message, "4");
    }

    #[test]
    fn test_broadcaster_send_and_recent() {
        let b = LogBroadcaster::new(5);
        b.send("INFO", "test", "hello".into());
        let recent = b.recent(10);
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].message, "hello");
    }

    /// 测试异步文件写入功能。
    /// 创建 LogBroadcaster 后发送日志，验证日志被写入到正确的文件。
    #[tokio::test]
    async fn test_log_file_writer() {
        use std::path::PathBuf;
        use tokio::fs;

        let temp_dir = std::env::temp_dir().join("fi-code-log-test");
        let _ = fs::remove_dir_all(&temp_dir).await;

        let writer = LogFileWriter::new(temp_dir.clone()).await.unwrap();
        let (tx, rx) = tokio::sync::mpsc::channel(10);
        let handle = tokio::spawn(writer.run(rx));

        // 发送 agent 日志
        tx.send(LogEntry {
            timestamp: "12:00:00".into(),
            level: "INFO".into(),
            module: "fi_code_core::agent".into(),
            message: "agent hello".into(),
        })
        .await
        .unwrap();

        // 发送 tui 日志
        tx.send(LogEntry {
            timestamp: "12:00:01".into(),
            level: "DEBUG".into(),
            module: "fi_code_tui::app".into(),
            message: "tui hello".into(),
        })
        .await
        .unwrap();

        // 等待写入
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

        // 关闭通道，让 writer 退出
        drop(tx);
        let _ = handle.await;

        // 验证 agent.log
        let agent_content = fs::read_to_string(temp_dir.join("agent.log"))
            .await
            .unwrap();
        assert!(agent_content.contains("agent hello"));
        assert!(agent_content.contains("fi_code_core::agent"));

        // 验证 tui.log
        let tui_content = fs::read_to_string(temp_dir.join("tui.log"))
            .await
            .unwrap();
        assert!(tui_content.contains("tui hello"));
        assert!(tui_content.contains("fi_code_tui::app"));

        // 清理
        let _ = fs::remove_dir_all(&temp_dir).await;
    }
}
