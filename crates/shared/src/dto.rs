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
// 共享 DTO（Data Transfer Objects）：前后端 API 传输用的结构体
// =============================================================================
// 本模块集中管理所有在 HTTP API / SSE 流 / JSON-RPC 中序列化传输的结构体，
// 确保前后端对同一数据结构的定义完全一致。

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use crate::enums::Role;

// -----------------------------------------------------------------------------
// Token 使用量统计
// -----------------------------------------------------------------------------

/// Token 使用量统计，用于 WaveMarker 等场景。
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
}

// -----------------------------------------------------------------------------
// 图片数据来源
// -----------------------------------------------------------------------------

/// 图片来源枚举，对应 Part::Image 的 source 字段。
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ImageSource {
    /// 本地文件系统路径
    Path { path: String },
    /// Base64 编码的图片数据
    Base64 { media_type: String, data: String },
    /// 远程图片 URL
    Url { url: String },
}

// -----------------------------------------------------------------------------
// 消息内容块（Part）：消息的原子组成单元
// -----------------------------------------------------------------------------

/// 内容块枚举：一条 `Message` 由多个 `Part` 按顺序组成。
///
/// 这种设计与 Anthropic / OpenAI 的最新内容块 API 对齐，
/// 支持纯文本、多模态图片、工具调用、工具结果、推理过程、
/// 波浪标记以及用量统计。
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
        /// 工具参数，使用 `serde_json::Value` 保持灵活性
        arguments: Value,
    },
    /// 工具执行结果（由 User 角色消息携带，回传给模型）
    ToolResult {
        tool_call_id: String,
        content: String,
        /// 工具执行耗时（毫秒），用于 TUI 展示性能信息
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
        /// 可选的签名，用于验证推理内容未被篡改
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
}

// -----------------------------------------------------------------------------
// 消息结构体（Message）
// -----------------------------------------------------------------------------

/// 对话消息结构体，用于在多轮对话中保存角色与内容块。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub session_id: String,
    pub role: Role,
    pub created_at: u64,
    pub parts: Vec<Part>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost: Option<f64>,
}

impl Message {
    /// 便捷构造方法，自动生成 ULID id 与当前时间戳。
    pub fn new(session_id: impl Into<String>, role: Role, parts: Vec<Part>) -> Self {
        Self {
            id: ulid::Ulid::new().to_string(),
            session_id: session_id.into(),
            role,
            created_at: current_timestamp_ms(),
            parts,
            token_count: None,
            cost: None,
        }
    }
}

/// 获取当前 Unix 时间戳（毫秒）。
/// 使用 `std::time::SystemTime` 避免引入额外依赖（如 chrono）。
pub fn current_timestamp_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

// -----------------------------------------------------------------------------
// MessageBuilder：用于从持久化记录流式重建 Message
// -----------------------------------------------------------------------------

/// 消息构造器，在 `load_session` 过程中暂存一个 Message 的中间状态。
pub struct MessageBuilder {
    pub id: String,
    pub session_id: String,
    pub role: Role,
    pub created_at: u64,
    pub parts: Vec<Part>,
}

impl MessageBuilder {
    pub fn new(
        id: impl Into<String>,
        session_id: impl Into<String>,
        role: Role,
        created_at: u64,
    ) -> Self {
        Self {
            id: id.into(),
            session_id: session_id.into(),
            role,
            created_at,
            parts: Vec::new(),
        }
    }

    /// 向当前消息追加一个 Part。
    pub fn add_part(&mut self, part: Part) {
        self.parts.push(part);
    }

    /// 完成消息构造，合并可选的 token_count 和 cost。
    pub fn finalize(self, token_count: Option<u64>, cost: Option<f64>) -> Message {
        Message {
            id: self.id,
            session_id: self.session_id,
            role: self.role,
            created_at: self.created_at,
            parts: self.parts,
            token_count,
            cost,
        }
    }
}

// -----------------------------------------------------------------------------
// SSE 事件类型
// -----------------------------------------------------------------------------

/// SSE 事件枚举，服务端通过 SSE 流向前端推送的各类事件。
/// 注意：序列化标签为 `"type"`（而非 `"event"`），与前端解析逻辑保持一致。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SseEvent {
    #[serde(rename = "message")]
    Message { content: String },
    #[serde(rename = "part")]
    Part { part: Part },
    #[serde(rename = "task_progress")]
    TaskProgress {
        plan_id: String,
        tasks: Vec<TaskProgressItem>,
    },
    #[serde(rename = "error")]
    Error { message: String },
    #[serde(rename = "done")]
    Done { session_id: String },
}

/// 任务计划中的单个任务项。
/// 注意：`status` 使用 `String` 而非枚举，避免 shared crate 依赖 tools 模块。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskProgressItem {
    pub id: String,
    pub name: String,
    pub status: String,
}

// -----------------------------------------------------------------------------
// JSON-RPC 请求/响应
// -----------------------------------------------------------------------------

/// JSON-RPC 2.0 请求体。
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
    pub id: Option<Value>,
}

/// JSON-RPC 2.0 响应体。
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
    pub id: Option<Value>,
}

/// JSON-RPC 2.0 错误对象。
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl JsonRpcResponse {
    pub fn success(result: Value, id: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: Some(result),
            error: None,
            id,
        }
    }

    pub fn error(code: i32, message: impl Into<String>, id: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
                data: None,
            }),
            id,
        }
    }
}

// -----------------------------------------------------------------------------
// 通用 API 响应包装
// -----------------------------------------------------------------------------

/// 通用 API 响应包装器，所有 JSON API 返回统一使用此结构。
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            code: None,
        }
    }

    pub fn error(message: impl Into<String>, code: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message.into()),
            code: Some(code.into()),
        }
    }
}

// -----------------------------------------------------------------------------
// TUI 共享类型
// -----------------------------------------------------------------------------

/// 主题预设，与 UI 框架无关的可序列化主题配置。
///
/// 颜色值使用 u32 存储（0xRRGGBB 格式），便于在不同模块间传递和通过 HTTP 序列化。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemePreset {
    pub name: String,
    pub description: String,
    pub bg_base: u32,
    pub bg_surface: u32,
    pub bg_overlay: u32,
    pub border: u32,
    pub text_primary: u32,
    pub text_secondary: u32,
    pub text_muted: u32,
    pub text_placeholder: u32,
    pub brand: u32,
    pub user: u32,
    pub success: u32,
    pub warning: u32,
    pub error: u32,
    pub selection_bg: u32,
    pub selection_fg: u32,
    pub accent_hover: u32,
}

impl ThemePreset {
    /// 返回所有内置主题预设。
    pub fn all_presets() -> Vec<Self> {
        vec![
            Self {
                name: "deep_ocean".into(),
                description: "Deep Ocean Dark".into(),
                bg_base: 0x0d1117,
                bg_surface: 0x161b22,
                bg_overlay: 0x1a2332,
                border: 0x30363d,
                text_primary: 0xc9d1d9,
                text_secondary: 0x8b949e,
                text_muted: 0x484f58,
                text_placeholder: 0x6e7681,
                brand: 0x39d0d8,
                user: 0xf0883e,
                success: 0x3fb950,
                warning: 0xd29922,
                error: 0xf85149,
                selection_bg: 0x264f78,
                selection_fg: 0xffffff,
                accent_hover: 0x58a6ff,
            },
            Self {
                name: "github_dark".into(),
                description: "GitHub Dark".into(),
                bg_base: 0x0d1117,
                bg_surface: 0x161b22,
                bg_overlay: 0x1a2332,
                border: 0x30363d,
                text_primary: 0xc9d1d9,
                text_secondary: 0x8b949e,
                text_muted: 0x484f58,
                text_placeholder: 0x6e7681,
                brand: 0x58a6ff,
                user: 0xf0883e,
                success: 0x3fb950,
                warning: 0xd29922,
                error: 0xf85149,
                selection_bg: 0x264f78,
                selection_fg: 0xffffff,
                accent_hover: 0x58a6ff,
            },
            Self {
                name: "dracula".into(),
                description: "Dracula — Purple high contrast".into(),
                bg_base: 0x282a36,
                bg_surface: 0x44475a,
                bg_overlay: 0x6272a4,
                border: 0x44475a,
                text_primary: 0xf8f8f2,
                text_secondary: 0xbfbfbf,
                text_muted: 0x6272a4,
                text_placeholder: 0x6272a4,
                brand: 0xbd93f9,
                user: 0xff79c6,
                success: 0x50fa7b,
                warning: 0xf1fa8c,
                error: 0xff5555,
                selection_bg: 0x44475a,
                selection_fg: 0xf8f8f2,
                accent_hover: 0x8be9fd,
            },
            Self {
                name: "nord".into(),
                description: "Nord — Arctic blue-gray".into(),
                bg_base: 0x2e3440,
                bg_surface: 0x3b4252,
                bg_overlay: 0x434c5e,
                border: 0x4c566a,
                text_primary: 0xd8dee9,
                text_secondary: 0xe5e9f0,
                text_muted: 0x4c566a,
                text_placeholder: 0x4c566a,
                brand: 0x88c0d0,
                user: 0xd08770,
                success: 0xa3be8c,
                warning: 0xebcb8b,
                error: 0xbf616a,
                selection_bg: 0x434c5e,
                selection_fg: 0xd8dee9,
                accent_hover: 0x5e81ac,
            },
            Self {
                name: "catppuccin_mocha".into(),
                description: "Catppuccin Mocha — Soft pastel dark".into(),
                bg_base: 0x1e1e2e,
                bg_surface: 0x313244,
                bg_overlay: 0x45475a,
                border: 0x585b70,
                text_primary: 0xcdd6f4,
                text_secondary: 0xbac2de,
                text_muted: 0x6c7086,
                text_placeholder: 0x6c7086,
                brand: 0xcba6f7,
                user: 0xfab387,
                success: 0xa6e3a1,
                warning: 0xf9e2af,
                error: 0xf38ba8,
                selection_bg: 0x585b70,
                selection_fg: 0xcdd6f4,
                accent_hover: 0x89b4fa,
            },
            Self {
                name: "catppuccin_frappe".into(),
                description: "Catppuccin Frappe — Muted dark".into(),
                bg_base: 0x303446,
                bg_surface: 0x414559,
                bg_overlay: 0x51576d,
                border: 0x626880,
                text_primary: 0xc6d0f5,
                text_secondary: 0xb5bfe2,
                text_muted: 0x737994,
                text_placeholder: 0x737994,
                brand: 0xca9ee6,
                user: 0xef9f76,
                success: 0xa6d189,
                warning: 0xe5c890,
                error: 0xe78284,
                selection_bg: 0x626880,
                selection_fg: 0xc6d0f5,
                accent_hover: 0x8caaee,
            },
            Self {
                name: "catppuccin_macchiato".into(),
                description: "Catppuccin Macchiato — Medium dark".into(),
                bg_base: 0x24273a,
                bg_surface: 0x363a4f,
                bg_overlay: 0x494d64,
                border: 0x5b6078,
                text_primary: 0xcad3f5,
                text_secondary: 0xb8c0e0,
                text_muted: 0x6e738d,
                text_placeholder: 0x6e738d,
                brand: 0xc6a0f6,
                user: 0xf5a97f,
                success: 0xa6da95,
                warning: 0xeed49f,
                error: 0xed8796,
                selection_bg: 0x5b6078,
                selection_fg: 0xcad3f5,
                accent_hover: 0x8aadf4,
            },
            Self {
                name: "tokyo_night".into(),
                description: "Tokyo Night — Deep blue neon".into(),
                bg_base: 0x1a1b26,
                bg_surface: 0x24283b,
                bg_overlay: 0x414868,
                border: 0x565f89,
                text_primary: 0xa9b1d6,
                text_secondary: 0xc0caf5,
                text_muted: 0x565f89,
                text_placeholder: 0x565f89,
                brand: 0x7aa2f7,
                user: 0xff9e64,
                success: 0x9ece6a,
                warning: 0xe0af68,
                error: 0xf7768e,
                selection_bg: 0x283457,
                selection_fg: 0xc0caf5,
                accent_hover: 0xbb9af7,
            },
            Self {
                name: "gruvbox_dark".into(),
                description: "Gruvbox Dark — Warm retro".into(),
                bg_base: 0x282828,
                bg_surface: 0x3c3836,
                bg_overlay: 0x504945,
                border: 0x665c54,
                text_primary: 0xebdbb2,
                text_secondary: 0xd5c4a1,
                text_muted: 0x928374,
                text_placeholder: 0x928374,
                brand: 0xb8bb26,
                user: 0xfe8019,
                success: 0x98971a,
                warning: 0xd79921,
                error: 0xcc241d,
                selection_bg: 0x504945,
                selection_fg: 0xebdbb2,
                accent_hover: 0x83a598,
            },
            Self {
                name: "one_dark".into(),
                description: "One Dark — Atom classic".into(),
                bg_base: 0x282c34,
                bg_surface: 0x3e4451,
                bg_overlay: 0x21252b,
                border: 0x5c6370,
                text_primary: 0xabb2bf,
                text_secondary: 0x828997,
                text_muted: 0x5c6370,
                text_placeholder: 0x5c6370,
                brand: 0x61afef,
                user: 0xe5c07b,
                success: 0x98c379,
                warning: 0xd19a66,
                error: 0xe06c75,
                selection_bg: 0x3e4451,
                selection_fg: 0xabb2bf,
                accent_hover: 0xc678dd,
            },
            Self {
                name: "monokai".into(),
                description: "Monokai — High saturation neon".into(),
                bg_base: 0x272822,
                bg_surface: 0x3e3d32,
                bg_overlay: 0x49483e,
                border: 0x75715e,
                text_primary: 0xf8f8f2,
                text_secondary: 0xcfcfc2,
                text_muted: 0x75715e,
                text_placeholder: 0x75715e,
                brand: 0xa6e22e,
                user: 0xfd971f,
                success: 0xa6e22e,
                warning: 0xe6db74,
                error: 0xf92672,
                selection_bg: 0x49483e,
                selection_fg: 0xf8f8f2,
                accent_hover: 0x66d9ef,
            },
            Self {
                name: "solarized_dark".into(),
                description: "Solarized Dark — Scientific low contrast".into(),
                bg_base: 0x002b36,
                bg_surface: 0x073642,
                bg_overlay: 0x586e75,
                border: 0x586e75,
                text_primary: 0x839496,
                text_secondary: 0x93a1a1,
                text_muted: 0x586e75,
                text_placeholder: 0x586e75,
                brand: 0x268bd2,
                user: 0xcb4b16,
                success: 0x859900,
                warning: 0xb58900,
                error: 0xdc322f,
                selection_bg: 0x073642,
                selection_fg: 0x93a1a1,
                accent_hover: 0x2aa198,
            },
            Self {
                name: "tomorrow_night".into(),
                description: "Tomorrow Night — Neutral dark gray".into(),
                bg_base: 0x1d1f21,
                bg_surface: 0x282a2e,
                bg_overlay: 0x373b41,
                border: 0x4d4d4d,
                text_primary: 0xc5c8c6,
                text_secondary: 0xb4b7b4,
                text_muted: 0x969896,
                text_placeholder: 0x969896,
                brand: 0x81a2be,
                user: 0xde935f,
                success: 0xb5bd68,
                warning: 0xf0c674,
                error: 0xcc6666,
                selection_bg: 0x373b41,
                selection_fg: 0xc5c8c6,
                accent_hover: 0xb294bb,
            },
            Self {
                name: "material_dark".into(),
                description: "Material Dark — Google Material".into(),
                bg_base: 0x263238,
                bg_surface: 0x37474f,
                bg_overlay: 0x455a64,
                border: 0x546e7a,
                text_primary: 0xb0bec5,
                text_secondary: 0xcfd8dc,
                text_muted: 0x78909c,
                text_placeholder: 0x78909c,
                brand: 0x80cbc4,
                user: 0xffab91,
                success: 0xa5d6a7,
                warning: 0xffe082,
                error: 0xef9a9a,
                selection_bg: 0x455a64,
                selection_fg: 0xeceff1,
                accent_hover: 0x81d4fa,
            },
            Self {
                name: "oceanic_next".into(),
                description: "Oceanic Next — Deep sea blue-green".into(),
                bg_base: 0x1b2b34,
                bg_surface: 0x343d46,
                bg_overlay: 0x4f5b66,
                border: 0x65737e,
                text_primary: 0xd8dee9,
                text_secondary: 0xc0c5ce,
                text_muted: 0x65737e,
                text_placeholder: 0x65737e,
                brand: 0x6699cc,
                user: 0xf99157,
                success: 0x99c794,
                warning: 0xfac863,
                error: 0xec5f67,
                selection_bg: 0x4f5b66,
                selection_fg: 0xd8dee9,
                accent_hover: 0xc594c5,
            },
            Self {
                name: "palenight".into(),
                description: "Palenight — Blue-purple night sky".into(),
                bg_base: 0x292d3e,
                bg_surface: 0x444267,
                bg_overlay: 0x32374d,
                border: 0x676e95,
                text_primary: 0xa6accd,
                text_secondary: 0x959dcb,
                text_muted: 0x676e95,
                text_placeholder: 0x676e95,
                brand: 0x82aaff,
                user: 0xffcb6b,
                success: 0xc3e88d,
                warning: 0xf78c6c,
                error: 0xff5370,
                selection_bg: 0x444267,
                selection_fg: 0xffffff,
                accent_hover: 0xc792ea,
            },
            Self {
                name: "night_owl".into(),
                description: "Night Owl — Deep blue for night coding".into(),
                bg_base: 0x011627,
                bg_surface: 0x0b2942,
                bg_overlay: 0x1d3b53,
                border: 0x2e4960,
                text_primary: 0xd6deeb,
                text_secondary: 0xabb2bf,
                text_muted: 0x5f7e97,
                text_placeholder: 0x5f7e97,
                brand: 0x82aaff,
                user: 0xf78c6c,
                success: 0xaddb67,
                warning: 0xecc48d,
                error: 0xef5350,
                selection_bg: 0x1d3b53,
                selection_fg: 0xd6deeb,
                accent_hover: 0xc792ea,
            },
            Self {
                name: "ayu_mirage".into(),
                description: "Ayu Mirage — Dark gray-blue modern".into(),
                bg_base: 0x1f2430,
                bg_surface: 0x2a3342,
                bg_overlay: 0x3d4752,
                border: 0x4d5768,
                text_primary: 0xcccac2,
                text_secondary: 0xb3b1ad,
                text_muted: 0x4d5768,
                text_placeholder: 0x4d5768,
                brand: 0x73b8ff,
                user: 0xff9940,
                success: 0x87d96c,
                warning: 0xf2d5cf,
                error: 0xf26d78,
                selection_bg: 0x3d4752,
                selection_fg: 0xcccac2,
                accent_hover: 0xdfbfff,
            },
            Self {
                name: "kanagawa".into(),
                description: "Kanagawa — Japanese ukiyo-e inspired".into(),
                bg_base: 0x1f1f28,
                bg_surface: 0x2a2a37,
                bg_overlay: 0x363646,
                border: 0x54546d,
                text_primary: 0xdcd7ba,
                text_secondary: 0xc8c093,
                text_muted: 0x727169,
                text_placeholder: 0x727169,
                brand: 0x7e9cd8,
                user: 0xff9e3b,
                success: 0x76946a,
                warning: 0xc0a36e,
                error: 0xc34043,
                selection_bg: 0x2d4f67,
                selection_fg: 0xdcd7ba,
                accent_hover: 0x957fb8,
            },
            Self {
                name: "solarized_light".into(),
                description: "Solarized Light — Scientific beige".into(),
                bg_base: 0xfdf6e3,
                bg_surface: 0xeee8d5,
                bg_overlay: 0x93a1a1,
                border: 0x839496,
                text_primary: 0x657b83,
                text_secondary: 0x586e75,
                text_muted: 0x93a1a1,
                text_placeholder: 0x93a1a1,
                brand: 0x268bd2,
                user: 0xcb4b16,
                success: 0x859900,
                warning: 0xb58900,
                error: 0xdc322f,
                selection_bg: 0xeee8d5,
                selection_fg: 0x586e75,
                accent_hover: 0x2aa198,
            },
            Self {
                name: "gruvbox_light".into(),
                description: "Gruvbox Light — Warm yellow retro".into(),
                bg_base: 0xfbf1c7,
                bg_surface: 0xebdbb2,
                bg_overlay: 0xd5c4a1,
                border: 0xbdae93,
                text_primary: 0x3c3836,
                text_secondary: 0x504945,
                text_muted: 0x928374,
                text_placeholder: 0x928374,
                brand: 0x79740e,
                user: 0xaf3a03,
                success: 0x79740e,
                warning: 0xb57614,
                error: 0x9d0006,
                selection_bg: 0xd5c4a1,
                selection_fg: 0x3c3836,
                accent_hover: 0x076678,
            },
            Self {
                name: "one_light".into(),
                description: "One Light — Atom light classic".into(),
                bg_base: 0xfafafa,
                bg_surface: 0xf0f0f0,
                bg_overlay: 0xe5e5e5,
                border: 0xd0d0d0,
                text_primary: 0x383a42,
                text_secondary: 0x696c77,
                text_muted: 0xa0a1a7,
                text_placeholder: 0xa0a1a7,
                brand: 0x4078f2,
                user: 0xc18401,
                success: 0x50a14f,
                warning: 0x986801,
                error: 0xe45649,
                selection_bg: 0xd7d7d7,
                selection_fg: 0x383a42,
                accent_hover: 0xa626a4,
            },
            Self {
                name: "catppuccin_latte".into(),
                description: "Catppuccin Latte — Soft cream light".into(),
                bg_base: 0xeff1f5,
                bg_surface: 0xe6e9ef,
                bg_overlay: 0xccd0da,
                border: 0xbcc0cc,
                text_primary: 0x4c4f69,
                text_secondary: 0x5c5f77,
                text_muted: 0x9ca0b0,
                text_placeholder: 0x9ca0b0,
                brand: 0x8839ef,
                user: 0xfe640b,
                success: 0x40a02b,
                warning: 0xdf8e1d,
                error: 0xd20f39,
                selection_bg: 0xccd0da,
                selection_fg: 0x4c4f69,
                accent_hover: 0x1e66f5,
            },
            Self {
                name: "ayu_light".into(),
                description: "Ayu Light — Minimal white".into(),
                bg_base: 0xfcfcfc,
                bg_surface: 0xf3f3f3,
                bg_overlay: 0xe8e8e8,
                border: 0xd0d0d0,
                text_primary: 0x5c6166,
                text_secondary: 0x8a9199,
                text_muted: 0xb0b0b0,
                text_placeholder: 0xb0b0b0,
                brand: 0xff9940,
                user: 0xfa8d3e,
                success: 0x86b300,
                warning: 0xf2ae49,
                error: 0xf07171,
                selection_bg: 0xe8e8e8,
                selection_fg: 0x5c6166,
                accent_hover: 0x399ee6,
            },
            Self {
                name: "papercolor".into(),
                description: "PaperColor — Paper white high readability".into(),
                bg_base: 0xeeeeee,
                bg_surface: 0xe0e0e0,
                bg_overlay: 0xd0d0d0,
                border: 0xc0c0c0,
                text_primary: 0x444444,
                text_secondary: 0x585858,
                text_muted: 0x808080,
                text_placeholder: 0x808080,
                brand: 0x005f87,
                user: 0xd75f00,
                success: 0x008700,
                warning: 0xaf8700,
                error: 0xaf0000,
                selection_bg: 0x0087af,
                selection_fg: 0xffffff,
                accent_hover: 0x8700af,
            },
            Self {
                name: "tango".into(),
                description: "Tango — GNOME terminal vivid".into(),
                bg_base: 0x2e3436,
                bg_surface: 0x3e4446,
                bg_overlay: 0x555753,
                border: 0x6e706b,
                text_primary: 0xd3d7cf,
                text_secondary: 0xbabdb6,
                text_muted: 0x888a85,
                text_placeholder: 0x888a85,
                brand: 0x3465a4,
                user: 0xf57900,
                success: 0x4e9a06,
                warning: 0xce5c00,
                error: 0xcc0000,
                selection_bg: 0x555753,
                selection_fg: 0xffffff,
                accent_hover: 0x75507b,
            },
            Self {
                name: "base16_dark".into(),
                description: "Base16 Dark — Architectural dark".into(),
                bg_base: 0x181818,
                bg_surface: 0x282828,
                bg_overlay: 0x383838,
                border: 0x585858,
                text_primary: 0xd8d8d8,
                text_secondary: 0xb8b8b8,
                text_muted: 0x585858,
                text_placeholder: 0x585858,
                brand: 0x7cafc2,
                user: 0xdc9656,
                success: 0xa1b56c,
                warning: 0xf7ca88,
                error: 0xab4642,
                selection_bg: 0x383838,
                selection_fg: 0xd8d8d8,
                accent_hover: 0xba8baf,
            },
            Self {
                name: "campbell".into(),
                description: "Campbell — Windows Terminal default".into(),
                bg_base: 0x0c0c0c,
                bg_surface: 0x1f1f1f,
                bg_overlay: 0x2f2f2f,
                border: 0x3f3f3f,
                text_primary: 0xcccccc,
                text_secondary: 0xb0b0b0,
                text_muted: 0x767676,
                text_placeholder: 0x767676,
                brand: 0x3b78ff,
                user: 0xf9f1a5,
                success: 0x13a10e,
                warning: 0xc19c00,
                error: 0xc50f1f,
                selection_bg: 0x2f2f2f,
                selection_fg: 0xffffff,
                accent_hover: 0x881798,
            },
            Self {
                name: "ubuntu".into(),
                description: "Ubuntu — Purple-orange classic".into(),
                bg_base: 0x300a24,
                bg_surface: 0x4e1942,
                bg_overlay: 0x6e2c5a,
                border: 0x8e3e72,
                text_primary: 0xeeeeee,
                text_secondary: 0xd3d3d3,
                text_muted: 0x878787,
                text_placeholder: 0x878787,
                brand: 0xe95420,
                user: 0xfb7c38,
                success: 0x38b44a,
                warning: 0xefb73e,
                error: 0xdf382c,
                selection_bg: 0x6e2c5a,
                selection_fg: 0xffffff,
                accent_hover: 0x77216f,
            },
            Self {
                name: "retro".into(),
                description: "Retro — Amber CRT simulation".into(),
                bg_base: 0x1a1a00,
                bg_surface: 0x2a2a00,
                bg_overlay: 0x3a3a00,
                border: 0x4a4a00,
                text_primary: 0xffb000,
                text_secondary: 0xcc8e00,
                text_muted: 0x996b00,
                text_placeholder: 0x996b00,
                brand: 0xffb000,
                user: 0xffcc00,
                success: 0x00ff00,
                warning: 0xffff00,
                error: 0xff3333,
                selection_bg: 0x4a4a00,
                selection_fg: 0xffb000,
                accent_hover: 0xff8000,
            },
            Self {
                name: "matrix".into(),
                description: "Matrix — Pure green hacker".into(),
                bg_base: 0x000000,
                bg_surface: 0x0d1f0d,
                bg_overlay: 0x1a331a,
                border: 0x267326,
                text_primary: 0x00ff41,
                text_secondary: 0x00cc33,
                text_muted: 0x008f11,
                text_placeholder: 0x008f11,
                brand: 0x00ff41,
                user: 0x00ff41,
                success: 0x00ff41,
                warning: 0x55ff55,
                error: 0xff0000,
                selection_bg: 0x1a331a,
                selection_fg: 0x00ff41,
                accent_hover: 0x00ff41,
            },
            Self {
                name: "cyberpunk".into(),
                description: "Cyberpunk — Neon pink-purple".into(),
                bg_base: 0x0a0014,
                bg_surface: 0x1a0033,
                bg_overlay: 0x2a004d,
                border: 0x4a0080,
                text_primary: 0xff00ff,
                text_secondary: 0xcc00cc,
                text_muted: 0x800080,
                text_placeholder: 0x800080,
                brand: 0xff00ff,
                user: 0xff6600,
                success: 0x00ff66,
                warning: 0xffff00,
                error: 0xff0044,
                selection_bg: 0x2a004d,
                selection_fg: 0xffffff,
                accent_hover: 0x00ffff,
            },
        ]
    }
}

/// 文件树节点。
#[derive(Debug, Clone)]
pub struct FileNode {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub depth: usize, // 缩进深度，用于层级可视化
}

/// 命令元数据，用于 TUI 命令列表展示。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandMeta {
    pub name: String,
    pub description: String,
    pub args_hint: Option<String>,
}

// -----------------------------------------------------------------------------
// Chat API 请求/响应 DTO
// -----------------------------------------------------------------------------

/// Chat 端点请求体。
#[derive(Deserialize)]
pub struct ChatRequest {
    pub session_id: Option<String>,
    pub message: String,
}

/// 模型切换请求体。
#[derive(Deserialize)]
pub struct SwitchModelRequest {
    pub provider: String,
    pub model: String,
    pub api_key: Option<String>,
}

/// 模型切换响应。
#[derive(Serialize)]
pub struct SwitchModelResponse {
    pub provider: String,
    pub model: String,
}

// -----------------------------------------------------------------------------
// Session API DTO
// -----------------------------------------------------------------------------

/// Session 信息传输对象。
#[derive(Debug, Serialize, Deserialize)]
pub struct SessionDto {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub last_active: String,
    pub message_count: usize,
    pub is_current: bool,
}

/// 创建 Session 请求。
#[derive(Debug, Deserialize)]
pub struct CreateSessionRequest {
    pub name: String,
    #[serde(default)]
    pub template: String,
}

/// 重命名 Session 请求。
#[derive(Debug, Deserialize)]
pub struct RenameSessionRequest {
    pub name: String,
}
