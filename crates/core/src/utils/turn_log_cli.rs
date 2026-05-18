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
// turn_log_cli 模块：CLI 读取、过滤、格式化 Turn 日志
// =============================================================================
// 提供 `fi-code-cli logs` 子命令的后端逻辑，读取 `~/.config/fi-code/logs/turns.jsonl`
// 并以人类可读格式或原始 JSON 输出到 stdout。

use anyhow::{Context, Result};
use serde_json::Value;
use std::path::PathBuf;

/// CLI 日志查看选项。
pub struct LogsOptions {
    pub limit: usize,
    pub follow: bool,
    pub session_filter: Option<String>,
    pub tool_filter: Option<String>,
    pub raw: bool,
}

/// 默认日志文件路径：`~/.config/fi-code/logs/turns.jsonl`。
pub fn default_log_path() -> PathBuf {
    directories::ProjectDirs::from("", "", "fi-code")
        .map(|d| d.config_dir().join("logs").join("turns.jsonl"))
        .unwrap_or_else(|| PathBuf::from(".config/fi-code/logs/turns.jsonl"))
}

/// 执行 logs CLI 命令。
pub fn run_logs_cli(options: LogsOptions) -> Result<()> {
    let path = default_log_path();
    if !path.exists() {
        anyhow::bail!("日志文件不存在: {}", path.display());
    }

    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("读取日志文件失败: {}", path.display()))?;

    let lines: Vec<&str> = content.lines().collect();
    let filtered: Vec<Value> = lines
        .iter()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|v| {
            if let Some(ref session) = options.session_filter {
                v.get("session_id")
                    .and_then(|s| s.as_str())
                    .map(|s| s.starts_with(session))
                    .unwrap_or(false)
            } else {
                true
            }
        })
        .filter(|v| {
            if let Some(ref tool) = options.tool_filter {
                v.get("tool_results")
                    .and_then(|arr| arr.as_array())
                    .map(|arr| {
                        arr.iter()
                            .any(|t| t.get("name").and_then(|n| n.as_str()) == Some(tool.as_str()))
                    })
                    .unwrap_or(false)
            } else {
                true
            }
        })
        .collect();

    let start = filtered.len().saturating_sub(options.limit);
    for entry in &filtered[start..] {
        if options.raw {
            println!("{}", serde_json::to_string_pretty(entry).unwrap());
        } else {
            print_formatted(entry);
        }
    }

    Ok(())
}

/// 以人类可读格式打印单条 Turn 日志。
fn print_formatted(entry: &Value) {
    let turn_idx = entry
        .get("turn_index")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let session_id = entry
        .get("session_id")
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let ts = entry
        .get("timestamp")
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let finish = entry
        .get("finish_reason")
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let usage = entry.get("token_usage").cloned().unwrap_or_default();
    let prompt = usage
        .get("prompt_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let completion = usage
        .get("completion_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    println!("═══════════════════════════════════════════════════");
    println!(
        "Turn #{} | Session: {}... | {}",
        turn_idx,
        &session_id[..session_id.len().min(8)],
        ts
    );
    println!("Finish: {} | Tokens: {}↑ {}↓", finish, prompt, completion);
    println!("───────────────────────────────────────────────────");

    // LLM 输出
    if let Some(blocks) = entry.get("content_blocks").and_then(|v| v.as_array()) {
        for block in blocks {
            if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
                if !text.trim().is_empty() {
                    println!("[LLM]\n{}", text);
                }
            }
        }
    }

    // 工具结果
    if let Some(results) = entry.get("tool_results").and_then(|v| v.as_array()) {
        for tr in results {
            let name = tr.get("name").and_then(|v| v.as_str()).unwrap_or("?");
            let duration = tr.get("duration_ms").and_then(|v| v.as_u64()).unwrap_or(0);
            let is_error = tr
                .get("is_error")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let icon = if is_error { "❌" } else { "✅" };
            let args = tr.get("arguments").cloned().unwrap_or_default();
            let content = tr.get("content").and_then(|v| v.as_str()).unwrap_or("");

            println!("\n[Tool] {} | {} {}ms", name, icon, duration);
            println!("参数: {}", serde_json::to_string(&args).unwrap());
            println!("结果: {}", content.chars().take(500).collect::<String>());
        }
    }

    println!("═══════════════════════════════════════════════════\n");
}
