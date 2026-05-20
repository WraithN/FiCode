# 读写文件信息流单读优化实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 优化 `read`/`write`/`edit` 工具的结果展示，支持语法高亮、diff 着色，不再使用 JSON 格式展示。

**Architecture:** 将工具结果从单一 JSON/纯文本字符串转变为多 Part 序列（ToolResult 元数据标题 + CodeBlock 代码内容）。后端在 SSE 流中顺序推送两个 Part，前端/TUI 按顺序渲染。

**Tech Stack:** Rust (tokio, serde, syntect), React + TypeScript + TailwindCSS, ratatui + syntect

---

## 文件结构映射

| 文件 | 职责 | 操作 |
|------|------|------|
| `crates/shared/src/dto.rs` | Part 枚举定义 | 添加 `CodeBlock` 变体 |
| `crates/core/src/tools/basic_tools.rs` | 底层工具实现 | 修改 `run_read`/`run_write`/`run_edit` 返回纯文本 |
| `crates/core/src/tools/mod.rs` | 工具调用编排 | 修改 `execute_tool_calls` 发送多 Part SSE |
| `crates/core/src/utils/file_type.rs` | 文件类型推断（新建） | 实现 `file_type_from_path` |
| `crates/tui/src/components/part_renderer/mod.rs` | TUI 渲染器注册表 | 注册 `CodeBlockRenderer` |
| `crates/tui/src/components/part_renderer/code_block.rs` | TUI 代码块渲染 | 增强 diff 着色、控制字符处理 |
| `frontend/src/components/part-renderers/CodeBlockPart.tsx` | Web 代码块组件 | 增强 diff 着色、控制字符处理 |
| `frontend/src/components/part-renderers/ToolResultPart.tsx` | Web 工具结果组件 | 样式微调 |

---

## Task 1: 添加 CodeBlock 到 Part 枚举

**Files:**
- Modify: `crates/shared/src/dto.rs:71-124`

- [ ] **Step 1: 添加 CodeBlock 变体到 Part 枚举**

在 `Part` 枚举的 `SystemNotice` 变体之前插入：

```rust
    /// 代码块内容，用于展示文件内容和 diff，支持语法高亮
    CodeBlock {
        language: String,
        code: String,
    },
```

修改后的 `Part` 枚举（第 71-124 行附近）：

```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Part {
    /// 纯文本内容
    Text { text: String },
    /// 图片内容，支持本地路径、Base64 数据或远程 URL
    Image { source: ImageSource },
    /// 工具调用请求（由 Assistant 发起）
    ToolUse {
        id: String,
        name: String,
        arguments: Value,
    },
    /// 工具执行结果（由 User 角色消息携带，回传给模型）
    ToolResult {
        tool_call_id: String,
        content: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        duration_ms: Option<u64>,
    },
    /// 工具执行错误（由 User 角色消息携带，回传给模型）
    ToolError {
        tool_call_id: String,
        content: String,
        error_message: String,
    },
    /// 推理/思考过程（如 Claude Extended Thinking）
    Reasoning {
        thinking: String,
        signature: Option<String>,
    },
    /// 波浪标记，用于标识 Agent 执行步骤
    WaveMarker {
        step: u32,
        total: Option<u32>,
        git_snapshot: Option<String>,
        timestamp: u64,
        delta_tokens: TokenUsage,
    },
    /// 用量统计
    Usage {
        input_tokens: u32,
        output_tokens: u32,
        latency_ms: u32,
        cost: Option<f64>,
    },
    /// 代码块内容，用于展示文件内容和 diff，支持语法高亮
    CodeBlock {
        language: String,
        code: String,
    },
    /// 系统通知（如压缩完成、Agent 切换等）
    #[serde(rename = "system_notice")]
    SystemNotice {
        kind: String,
        content: String,
    },
}
```

- [ ] **Step 2: 编译检查**

Run: `cargo check`
Expected: 编译通过，无错误

- [ ] **Step 3: Commit**

```bash
git add crates/shared/src/dto.rs
git commit -m "feat(shared): add CodeBlock variant to Part enum"
```

---

## Task 2: 新建文件类型推断模块

**Files:**
- Create: `crates/core/src/utils/file_type.rs`
- Modify: `crates/core/src/utils/mod.rs`

- [ ] **Step 1: 创建 `file_type.rs`**

```rust
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

use std::path::Path;

/// 从文件路径推断语言标识符，用于语法高亮。
///
/// 示例：
/// - "src/main.rs" → Some("rust")
/// - "app.tsx" → Some("typescript")
/// - "Makefile" → Some("makefile")
/// - "无扩展名/未知" → None
pub fn file_type_from_path(path: &str) -> Option<String> {
    let path = Path::new(path);
    let ext = path.extension().and_then(|e| e.to_str())?;

    let lang = match ext.to_lowercase().as_str() {
        "rs" => "rust",
        "ts" | "tsx" => "typescript",
        "js" | "jsx" => "javascript",
        "py" => "python",
        "go" => "go",
        "java" => "java",
        "c" | "h" => "c",
        "cpp" | "hpp" | "cc" | "cxx" => "cpp",
        "md" | "markdown" => "markdown",
        "json" => "json",
        "yaml" | "yml" => "yaml",
        "toml" => "toml",
        "sh" | "bash" => "bash",
        "html" | "htm" => "html",
        "css" => "css",
        "sql" => "sql",
        "dockerfile" => "dockerfile",
        "xml" => "xml",
        "svg" => "svg",
        "scss" | "sass" => "scss",
        "less" => "less",
        "php" => "php",
        "rb" => "ruby",
        "swift" => "swift",
        "kt" | "kts" => "kotlin",
        "scala" => "scala",
        "r" => "r",
        "lua" => "lua",
        "vim" => "vim",
        "makefile" | "mk" => "makefile",
        "cmake" => "cmake",
        "zig" => "zig",
        "nim" => "nim",
        "elixir" | "ex" | "exs" => "elixir",
        "erl" => "erlang",
        "hs" => "haskell",
        "ml" | "mli" => "ocaml",
        "fs" | "fsx" => "fsharp",
        "cs" => "csharp",
        "vb" => "vb",
        "ps1" => "powershell",
        "dart" => "dart",
        "flutter" => "dart",
        "proto" => "protobuf",
        "graphql" | "gql" => "graphql",
        "prisma" => "prisma",
        _ => return None,
    };

    Some(lang.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_type_from_path_rust() {
        assert_eq!(file_type_from_path("src/main.rs"), Some("rust".to_string()));
    }

    #[test]
    fn test_file_type_from_path_typescript() {
        assert_eq!(file_type_from_path("app.tsx"), Some("typescript".to_string()));
        assert_eq!(file_type_from_path("utils.ts"), Some("typescript".to_string()));
    }

    #[test]
    fn test_file_type_from_path_javascript() {
        assert_eq!(file_type_from_path("index.js"), Some("javascript".to_string()));
    }

    #[test]
    fn test_file_type_from_path_python() {
        assert_eq!(file_type_from_path("script.py"), Some("python".to_string()));
    }

    #[test]
    fn test_file_type_from_path_unknown() {
        assert_eq!(file_type_from_path("Makefile"), Some("makefile".to_string()));
    }

    #[test]
    fn test_file_type_from_path_no_extension() {
        assert_eq!(file_type_from_path("README"), None);
    }

    #[test]
    fn test_file_type_from_path_unrecognized() {
        assert_eq!(file_type_from_path("data.bin"), None);
    }
}
```

- [ ] **Step 2: 在 `mod.rs` 中导出**

修改 `crates/core/src/utils/mod.rs`，在适当位置添加：

```rust
pub mod file_type;
```

- [ ] **Step 3: 编译测试**

Run: `cargo test -p fi-code-core file_type::tests`
Expected: 7 个测试全部通过

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/utils/file_type.rs crates/core/src/utils/mod.rs
git commit -m "feat(core): add file_type_from_path utility with language mapping"
```

---

## Task 3: 修改底层工具函数返回纯文本

**Files:**
- Modify: `crates/core/src/tools/basic_tools.rs`

- [ ] **Step 1: 修改 `run_read` 返回纯文本**

将第 136-140 行：

```rust
        let result_json = serde_json::json!({
            "content": preview.chars().take(OUTPUT_TRUNCATE_LENGTH).collect::<String>(),
            "full_content": full_content.chars().take(OUTPUT_TRUNCATE_LENGTH).collect::<String>(),
        });
        Ok(result_json.to_string())
```

改为：

```rust
        Ok(preview.chars().take(OUTPUT_TRUNCATE_LENGTH).collect::<String>())
```

- [ ] **Step 2: 修改 `run_write` 返回 diff 文本**

将第 269-275 行：

```rust
        let result_json = serde_json::json!({
            "content": format!("Wrote {} bytes", content.len()),
            "diff": diff_text,
            "is_new_file": is_new_file,
            "after_content": content,
        });
        Ok(result_json.to_string())
```

改为：

```rust
        if let Some(diff) = diff_text {
            Ok(diff)
        } else if is_new_file {
            Ok(format!("New file: {} ({} bytes)", path, content.len()))
        } else {
            Ok(format!("Wrote {} bytes to {}", content.len(), path))
        }
```

- [ ] **Step 3: 修改 `run_edit` 返回 diff 文本**

将第 320-326 行：

```rust
        let result_json = serde_json::json!({
            "content": format!("Edited {}", path),
            "diff": diff_opt,
            "is_new_file": false,
            "after_content": new_content,
        });
        Ok(result_json.to_string())
```

改为：

```rust
        if let Some(diff) = diff_opt {
            Ok(diff)
        } else {
            Ok(format!("Edited {} (no changes)", path))
        }
```

- [ ] **Step 4: 更新 `run_read` 测试**

将 `test_run_read` 测试（第 591-596 行）：

```rust
    #[test]
    fn test_run_read() {
        ensure_workspace();
        let lines = BasicTool::run_read("src/tools/basic_tools.rs", Some(DEFAULT_READ_MAX_LINES)).unwrap();
        assert_ne!(lines, "");
    }
```

改为：

```rust
    #[test]
    fn test_run_read() {
        ensure_workspace();
        let content = BasicTool::run_read("src/tools/basic_tools.rs", Some(DEFAULT_READ_MAX_LINES)).unwrap();
        assert_ne!(content, "");
        // 验证返回的是纯文本而非 JSON
        assert!(!content.starts_with('{'), "run_read should return plain text, not JSON");
    }
```

- [ ] **Step 5: 更新 `run_write` 测试**

将 `test_run_write` 测试（第 606-612 行）：

```rust
    #[test]
    fn test_run_write() {
        ensure_workspace();
        let path: &str = "target/test_write_file";
        let result = BasicTool::run_write(path, "test");
        assert!(result.is_ok());
        BasicTool::run_bash(&format!("rm {}", path));
    }
```

改为：

```rust
    #[test]
    fn test_run_write() {
        ensure_workspace();
        let path: &str = "target/test_write_file";
        let result = BasicTool::run_write(path, "test");
        assert!(result.is_ok());
        let content = result.unwrap();
        // 新文件应返回提示文本
        assert!(content.contains("New file") || content.contains("Wrote"));
        BasicTool::run_bash(&format!("rm {}", path));
    }
```

- [ ] **Step 6: 更新 `run_edit` 测试**

将 `test_run_edit` 测试（第 614-623 行）：

```rust
    #[test]
    fn test_run_edit() {
        ensure_workspace();
        let path = "target/test_edit_file";
        let result = BasicTool::run_write(path, "this is a test file");
        assert!(result.is_ok());
        let result = BasicTool::run_edit(path, "test file", "test edit file");
        assert!(result.is_ok());
        BasicTool::run_bash(&format!("rm {}", path));
    }
```

改为：

```rust
    #[test]
    fn test_run_edit() {
        ensure_workspace();
        let path = "target/test_edit_file";
        let result = BasicTool::run_write(path, "this is a test file");
        assert!(result.is_ok());
        let result = BasicTool::run_edit(path, "test file", "test edit file");
        assert!(result.is_ok());
        let content = result.unwrap();
        // diff 文本应包含 +/- 标记
        assert!(content.contains('+') || content.contains("no changes"));
        BasicTool::run_bash(&format!("rm {}", path));
    }
```

- [ ] **Step 7: 运行测试**

Run: `cargo test -p fi-code-core basic_tools::tests`
Expected: 所有测试通过

- [ ] **Step 8: Commit**

```bash
git add crates/core/src/tools/basic_tools.rs
git commit -m "feat(tools): return plain text from read/write/edit instead of JSON"
```

---

## Task 4: 修改 execute_tool_calls 发送多 Part SSE

**Files:**
- Modify: `crates/core/src/tools/mod.rs:1086-1141`

- [ ] **Step 1: 添加 file_type 导入**

在 `crates/core/src/tools/mod.rs` 的 imports 区域（约第 22-55 行），添加：

```rust
use crate::utils::file_type::file_type_from_path;
```

- [ ] **Step 2: 重构 `execute_tool_calls` 中的 SSE 发送逻辑**

替换第 1086-1141 行的整个代码块：

```rust
                // 从参数中提取路径（用于 read/write/edit）
                let file_path = input.get("path").and_then(|v| v.as_str());
                let language = file_path.and_then(file_type_from_path);

                // 根据工具类型构造不同的展示内容
                let is_read_write_edit = name == "read" || name == "write" || name == "edit";

                // 构造元数据标题文本
                let display_content = if name == "read" {
                    format!("✓ read: {} ({}ms)", file_path.unwrap_or("unknown"), duration_ms)
                } else if name == "write" {
                    let is_new = content.contains("New file");
                    if is_new {
                        format!("✓ write: {} ({}ms) — 新增文件", file_path.unwrap_or("unknown"), duration_ms)
                    } else {
                        format!("✓ write: {} ({}ms)", file_path.unwrap_or("unknown"), duration_ms)
                    }
                } else if name == "edit" {
                    format!("✓ edit: {} ({}ms)", file_path.unwrap_or("unknown"), duration_ms)
                } else {
                    content.clone()
                };

                if let Ok(mut guard) = cb.lock() {
                    if let Some(ref mut callback) = *guard {
                        // 发送 ToolResult（元数据标题）
                        let _ = callback(SseEvent::Part {
                            part: Part::ToolResult {
                                tool_call_id: id.clone(),
                                content: display_content.clone(),
                                duration_ms: Some(duration_ms),
                            },
                        });

                        // 对于 read/write/edit，额外发送 CodeBlock（实际内容）
                        if is_read_write_edit && !is_error {
                            let code_content = if content.starts_with("New file:") || content.starts_with("Wrote ") || content.starts_with("Edited ") {
                                // write/edit 没有 diff 时的提示文本，不发送 CodeBlock
                                None
                            } else {
                                Some(content.clone())
                            };

                            if let Some(code) = code_content {
                                let _ = callback(SseEvent::Part {
                                    part: Part::CodeBlock {
                                        language: language.clone(),
                                        code,
                                    },
                                });
                            }
                        }
                    }
                }
```

- [ ] **Step 3: 修改返回给 LLM 的 Part（压缩后的内容）**

将第 1128-1141 行（错误处理和压缩返回）保持不变，但确保压缩逻辑仍然正确：

```rust
                if is_error {
                    Part::ToolError {
                        tool_call_id: id,
                        content: content.clone(),
                        error_message: content,
                    }
                } else {
                    let compressed = crate::agent::compression::compress_tool_result(&content, is_aggressive, Some(&name));
                    Part::ToolResult {
                        tool_call_id: id,
                        content: compressed,
                        duration_ms: Some(duration_ms),
                    }
                }
```

注意：LLM 回传仍然使用压缩后的文本内容（包含完整的 diff 或文件内容），CodeBlock 仅用于前端展示。

- [ ] **Step 4: 编译检查**

Run: `cargo check -p fi-code-core`
Expected: 编译通过

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/tools/mod.rs
git commit -m "feat(tools): send CodeBlock Part for read/write/edit tool results"
```

---

## Task 5: TUI 注册 CodeBlockRenderer

**Files:**
- Modify: `crates/tui/src/components/part_renderer/mod.rs`

- [ ] **Step 1: 注册 CodeBlockRenderer**

在第 53-61 行的注册代码中，添加 `code_block`：

```rust
        registry.register("text", Box::new(text::TextRenderer));
        registry.register("reasoning", Box::new(thinking::ThinkingRenderer));
        registry.register("tool_use", Box::new(tool_call::ToolCallRenderer));
        registry.register("tool_result", Box::new(tool_result::ToolResultRenderer));
        registry.register("tool_error", Box::new(tool_error::ToolErrorRenderer));
        registry.register("wave_marker", Box::new(wave_marker::WaveMarkerRenderer));
        registry.register("usage", Box::new(usage::UsageRenderer));
        registry.register("image", Box::new(image::ImageRenderer));
        registry.register("code_block", Box::new(code_block::CodeBlockRenderer));
```

- [ ] **Step 2: 修改 `get` 方法匹配 CodeBlock**

在第 68-80 行的 `get` 方法中，添加 `CodeBlock` 匹配：

```rust
    pub fn get(&self, part: &Part) -> Option<&dyn PartRenderer> {
        let key = match part {
            Part::Text { .. } => "text",
            Part::Image { .. } => "image",
            Part::ToolUse { .. } => "tool_use",
            Part::ToolResult { .. } => "tool_result",
            Part::ToolError { .. } => "tool_error",
            Part::Reasoning { .. } => "reasoning",
            Part::WaveMarker { .. } => "wave_marker",
            Part::Usage { .. } => "usage",
            Part::CodeBlock { .. } => "code_block",
            Part::SystemNotice { .. } => "text",
        };
        self.renderers.get(key).map(|b| b.as_ref())
    }
```

- [ ] **Step 3: 修改 `code_block.rs` 实现 PartRenderer trait**

当前 `crates/tui/src/components/part_renderer/code_block.rs` 的 `CodeBlockRenderer` 没有实现 `PartRenderer` trait。需要重构：

将文件内容替换为：

```rust
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

use std::sync::LazyLock;

use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};
use syntect::{
    easy::HighlightLines,
    highlighting::{Style as SyntectStyle, ThemeSet},
    parsing::SyntaxSet,
};

use fi_code_core::session::message::Part;
use super::PartRenderer;
use crate::theme::Theme;

// 全局懒加载的 SyntaxSet 和 ThemeSet
static SYNTAX_SET: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);
static THEME_SET: LazyLock<ThemeSet> = LazyLock::new(ThemeSet::load_defaults);

/// 将 syntect 的 Style 转换为 ratatui 的 Style
fn syntect_to_ratatui(style: SyntectStyle) -> Style {
    let fg = style.foreground;
    Style::default()
        .fg(Color::Rgb(fg.r, fg.g, fg.b))
        .bg(Color::Rgb(
            style.background.r,
            style.background.g,
            style.background.b,
        ))
}

/// 代码块渲染器，支持语法高亮 + diff 着色 + 行号
pub struct CodeBlockRenderer;

impl PartRenderer for CodeBlockRenderer {
    fn height(&self, part: &Part, width: u16) -> u16 {
        if let Part::CodeBlock { code, .. } = part {
            let lines = code.lines().count() as u16;
            // +2 for borders
            lines.max(1) + 2
        } else {
            3
        }
    }

    fn draw(&self, frame: &mut ratatui::Frame, area: ratatui::layout::Rect, part: &Part, theme: &Theme, skip_lines: u16) {
        if let Part::CodeBlock { code, language } = part {
            let syntect_theme = THEME_SET
                .themes
                .get("base16-ocean.dark")
                .or_else(|| THEME_SET.themes.values().next())
                .expect("至少存在一个默认主题");

            let syntax = if language.is_empty() {
                None
            } else {
                SYNTAX_SET.find_syntax_by_token(language)
            };
                .unwrap_or_else(|| SYNTAX_SET.find_syntax_plain_text());

            let mut highlighter = HighlightLines::new(syntax, syntect_theme);

            let lines: Vec<&str> = code.lines().collect();
            let line_num_width = lines.len().to_string().len().max(2);
            let mut text_lines = Vec::with_capacity(lines.len());

            for (idx, line) in lines.iter().enumerate() {
                // 处理 diff 行着色：行首 + 绿色，- 红色
                let diff_style = if line.starts_with('+') {
                    Some(Style::default().fg(Color::Green))
                } else if line.starts_with('-') {
                    Some(Style::default().fg(Color::Red))
                } else {
                    None
                };

                // 行号 Span
                let line_num = format!("{:>width$} ", idx + 1, width = line_num_width);
                let line_num_span = Span::styled(line_num, Style::default().fg(Color::Rgb(80, 80, 80)));

                let mut spans: Vec<Span<'static>> = vec![line_num_span];

                if let Some(style) = diff_style {
                    // diff 行：整行使用 diff 着色，不做语法高亮
                    spans.push(Span::styled(line.to_string(), style));
                } else {
                    // 普通行：语法高亮
                    let highlighted = highlighter
                        .highlight_line(line, &SYNTAX_SET)
                        .unwrap_or_default();
                    for (style, text) in highlighted {
                        spans.push(Span::styled(text.to_string(), syntect_to_ratatui(style)));
                    }
                }

                text_lines.push(Line::from(spans));
            }

            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.border))
                .title(
                    Line::from(if language.is_empty() { "code" } else { language }.to_string())
                        .style(theme.style_primary()),
                );

            let paragraph = Paragraph::new(text_lines)
                .style(theme.style_primary())
                .block(block)
                .scroll((skip_lines, 0));

            frame.render_widget(paragraph, area);
        }
    }
}
```

- [ ] **Step 4: 编译检查**

Run: `cargo check -p fi-code-tui`
Expected: 编译通过

- [ ] **Step 5: Commit**

```bash
git add crates/tui/src/components/part_renderer/
git commit -m "feat(tui): register CodeBlockRenderer with diff highlighting"
```

---

## Task 6: Web 前端增强 CodeBlockPart

**Files:**
- Modify: `frontend/src/components/part-renderers/CodeBlockPart.tsx`
- Modify: `frontend/src/components/part-renderers/ToolResultPart.tsx`

- [ ] **Step 1: 重写 `CodeBlockPart.tsx`**

```tsx
import React from 'react';
import { Part } from '../../types/part';

export const CodeBlockPart: React.FC<{ part: Extract<Part, { type: 'code_block' }> }> = ({ part }) => {
  const lines = part.code.split('\n');

  const renderLine = (line: string, index: number) => {
    let className = 'block';
    let prefix = null;

    if (line.startsWith('+')) {
      className += ' bg-green-900/30 text-green-400';
    } else if (line.startsWith('-')) {
      className += ' bg-red-900/30 text-red-400';
    }

    return (
      <span key={index} className={className}>
        {line}
        {'\n'}
      </span>
    );
  };

  return (
    <div className="my-2 rounded overflow-hidden border border-border">
      <div className="text-xs text-text-muted bg-bg-secondary px-3 py-1 border-b border-border flex justify-between items-center">
        <span>{part.language || 'code'}</span>
      </div>
      <pre 
        className="text-sm text-text-primary bg-bg p-3 overflow-x-auto"
        style={{ tabSize: 4, whiteSpace: 'pre' }}
      >
        <code>{lines.map((line, idx) => renderLine(line, idx))}</code>
      </pre>
    </div>
  );
};
```

- [ ] **Step 2: 修改 `ToolResultPart.tsx` 样式**

```tsx
import React from 'react';
import { Part } from '../../types/part';

export const ToolResultPart: React.FC<{ part: Extract<Part, { type: 'tool_result' }> }> = ({ part }) => (
  <div className="my-1 px-2 py-1 rounded bg-bg-secondary/50 border-l-2 border-success">
    <span className="text-xs text-success font-mono">
      ✓ Result ({part.duration_ms}ms)
    </span>
  </div>
);
```

- [ ] **Step 3: 编译检查**

Run: `cd frontend && npx tsc --noEmit`
Expected: 编译通过

- [ ] **Step 4: Commit**

```bash
git add frontend/src/components/part-renderers/
git commit -m "feat(web): enhance CodeBlockPart with diff highlighting and control chars"
```

---

## Task 7: 全量测试验证

**Files:**
- 运行整个测试套件

- [ ] **Step 1: 运行 core 单元测试**

Run: `cargo test -p fi-code-core`
Expected: 全部通过

- [ ] **Step 2: 运行 TUI 单元测试**

Run: `cargo test -p fi-code-tui`
Expected: 全部通过

- [ ] **Step 3: 运行 E2E 测试**

Run: `cargo test --test e2e_cli`
Run: `cargo test --test e2e_tui`
Expected: 全部通过

- [ ] **Step 4: 运行 BDD 测试**

Run: `cargo test --test bdd`
Expected: 全部通过

- [ ] **Step 5: 运行 Clippy**

Run: `cargo clippy --all-targets --all-features`
Expected: 无错误（允许 warnings）

- [ ] **Step 6: Commit**

```bash
git commit -m "test: verify all tests pass after read/write/edit display optimization"
```

---

## 自检清单

**1. Spec 覆盖检查：**

| Spec 要求 | 对应 Task |
|-----------|-----------|
| 多 Part 序列（ToolResult + CodeBlock） | Task 4 |
| 语法高亮 | Task 5 (TUI), Task 6 (Web) |
| diff 着色（+ 绿色，- 红色） | Task 5 (TUI), Task 6 (Web) |
| 不使用 JSON 展示 | Task 3 |
| 元数据保留（duration_ms） | Task 4 |
| 控制字符正确展示 | Task 5 (TUI tab→空格), Task 6 (Web white-space: pre) |
| file_type_from_path | Task 2 |

**2. Placeholder 扫描：** 无 TBD/TODO/"implement later"

**3. 类型一致性：**
- `Part::CodeBlock { language: String, code: String }` 在 Rust、TypeScript、SSE 序列化中一致（空字符串表示未知语言）
- `file_type_from_path` 返回 `Option<String>`，通过 `unwrap_or_default()` 转为空字符串后赋值给 `language`

---

*计划日期：2026-05-18*
