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
// 共享常量：项目中的魔法值统一集中管理
// =============================================================================
// 本模块收集所有跨模块/前后端共享的硬编码数值，避免魔法值散布在代码各处。
// 按功能域分组，便于查找和维护。

// -----------------------------------------------------------------------------
// Agent 循环相关
// -----------------------------------------------------------------------------

/// Agent 单轮对话的最大轮数限制
pub const MAX_TURNS: usize = 25;

/// 单轮对话失败后的最大重试次数
pub const MAX_RUN_ONE_TURN_RETRIES: u32 = 1;

/// 发送给 LLM 的最大上下文消息数（用于截断历史）
pub const MAX_CONTEXT_MESSAGES: usize = 30;

/// Agent 循环中 run_one_turn 重试前的等待时间（秒）
pub const RUN_ONE_TURN_RETRY_DELAY_SECS: u64 = 2;

// -----------------------------------------------------------------------------
// 工具调用相关
// -----------------------------------------------------------------------------

/// 工具调用失败后的最大重试次数
pub const MAX_TOOL_RETRIES: u32 = 3;

/// 工具调用重试间隔（毫秒）
pub const TOOL_RETRY_DELAY_MS: u64 = 200;

/// Bash 命令执行超时时间（秒）
pub const BASH_TIMEOUT_SECS: u64 = 120;

/// 工具输出内容截断长度（字符数）
pub const OUTPUT_TRUNCATE_LENGTH: usize = 50_000;

/// grep 工具最大匹配结果数
pub const MAX_GREP_MATCHES: usize = 500;

// -----------------------------------------------------------------------------
// HTTP / API 请求相关
// -----------------------------------------------------------------------------

/// 默认请求总超时（毫秒）
pub const DEFAULT_TIMEOUT_MS: u64 = 300_000;

/// 默认 Chunk 读取超时（毫秒）
pub const DEFAULT_CHUNK_TIMEOUT_MS: u64 = 10_000;

/// 默认 TCP 连接超时（秒）
pub const DEFAULT_CONNECT_TIMEOUT_SECS: u64 = 30;

/// TUI HTTP 客户端连接超时（秒）
pub const TUI_CONNECT_TIMEOUT_SECS: u64 = 10;

/// TUI HTTP 客户端总超时（秒）
pub const TUI_TIMEOUT_SECS: u64 = 300;

/// 本地 Ollama 默认端口
pub const LOCALHOST_OLLAMA_PORT: u16 = 11434;

// -----------------------------------------------------------------------------
// SSE / 流式响应相关
// -----------------------------------------------------------------------------

/// SSE 通道缓冲区大小
pub const SSE_CHANNEL_BUFFER_SIZE: usize = 128;

// -----------------------------------------------------------------------------
// 重试退避策略相关
// -----------------------------------------------------------------------------

/// 重试基础延迟（毫秒）
pub const RETRY_BASE_DELAY_MS: u64 = 500;

/// 重试最大延迟（秒）
pub const RETRY_MAX_DELAY_SECS: u64 = 30;

/// 重试最小延迟（毫秒）
pub const RETRY_MIN_DELAY_MS: u64 = 100;

/// 重试指数退避的最大幂次（2^6 = 64）
pub const RETRY_MAX_EXPONENT: u32 = 6;

// -----------------------------------------------------------------------------
// 服务器相关
// -----------------------------------------------------------------------------

/// 默认 HTTP 服务监听端口
pub const DEFAULT_SERVER_PORT: u16 = 4040;

/// Session 超时时间（分钟）
pub const SESSION_TIMEOUT_MINUTES: u64 = 30;

// -----------------------------------------------------------------------------
// TUI 相关
// -----------------------------------------------------------------------------

/// TUI 渲染刷新间隔（毫秒，约 25 FPS）
pub const TUI_RENDER_INTERVAL_MS: u64 = 40;

/// TUI Log 窗口最大行数
pub const MAX_LOG_LINES: usize = 5000;

/// TUI 状态栏进度条宽度
pub const PROGRESS_BAR_WIDTH: usize = 20;

/// TUI 状态栏上下文条宽度
pub const CTX_BAR_WIDTH: usize = 10;

/// 默认上下文 Token 上限
pub const DEFAULT_CTX_LIMIT: usize = 128_000;

/// TUI 日志流断开重连等待时间（秒）
pub const LOG_RECONNECT_DELAY_SECS: u64 = 2;

/// TUI 启动时连接服务器最大等待时间（秒）
pub const TUI_SERVER_STARTUP_WAIT_SECS: u64 = 10;

/// TUI 启动时轮询间隔（毫秒）
pub const TUI_STARTUP_POLL_INTERVAL_MS: u64 = 500;

/// TUI 输入框子菜单最大可见项数
pub const MAX_VISIBLE_SUBMENU_ITEMS: u16 = 8;

// -----------------------------------------------------------------------------
// UI 动画相关
// -----------------------------------------------------------------------------

/// TUI Spinner 动画帧序列（Braille 点阵字符）
pub const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

// -----------------------------------------------------------------------------
// 文件读写相关
// -----------------------------------------------------------------------------

/// read 工具默认读取的最大行数
pub const DEFAULT_READ_MAX_LINES: usize = 10_000;
