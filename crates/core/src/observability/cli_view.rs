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

//! observability::cli_view：fi-code-cli `logs` 子命令后端
//!
//! 从 spans.jsonl 中按 session / limit 过滤并打印 span 记录，
//! 取代旧的 TurnLogger turns.jsonl 视图。

use anyhow::Result;
use std::path::PathBuf;

/// 默认 spans.jsonl 文件名（与 exporter 写出文件保持一致）。
const SPANS_FILENAME: &str = "spans.jsonl";

/// status 行的 type 标记（exporter 用此区分 span 记录与状态补丁）。
const LINE_TYPE_STATUS: &str = "status";

/// CLI logs 子命令选项。
pub struct LogsOptions {
    /// 显式指定 spans.jsonl 路径；为 None 时使用 default_log_path()
    pub file: Option<PathBuf>,
    /// 仅显示指定 session 的 span
    pub session: Option<String>,
    /// 最大显示条数
    pub limit: Option<usize>,
}

/// 默认 spans.jsonl 路径：`~/.config/fi-code/logs/spans.jsonl`
pub fn default_log_path() -> PathBuf {
    directories::ProjectDirs::from("", "", "fi-code")
        .map(|p| p.config_dir().join("logs").join(SPANS_FILENAME))
        .unwrap_or_else(|| PathBuf::from(SPANS_FILENAME))
}

/// 执行 `logs` CLI：读取 spans.jsonl 并按过滤条件打印。
pub fn run_logs_cli(opts: LogsOptions) -> Result<()> {
    let path = opts.file.unwrap_or_else(default_log_path);
    if !path.exists() {
        eprintln!("No spans.jsonl found at {:?}", path);
        return Ok(());
    }
    let content = std::fs::read_to_string(&path)?;
    let mut shown = 0usize;
    for line in content.lines() {
        // 卫子句：超出 limit 直接退出
        if let Some(limit) = opts.limit {
            if shown >= limit {
                break;
            }
        }
        let v: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        // 跳过 status patch 行：这些行用于覆盖前序 span 的最终状态，不是独立记录
        if v.get("type").and_then(|t| t.as_str()) == Some(LINE_TYPE_STATUS) {
            continue;
        }
        let session = v
            .pointer("/attributes/langfuse.session.id")
            .and_then(|s| s.as_str())
            .unwrap_or("");
        if let Some(filter) = opts.session.as_deref() {
            if session != filter {
                continue;
            }
        }
        let name = v.get("name").and_then(|n| n.as_str()).unwrap_or("");
        let trace_id = v.get("trace_id").and_then(|t| t.as_str()).unwrap_or("");
        println!("[{}] {} session={}", trace_id, name, session);
        shown += 1;
    }
    Ok(())
}
