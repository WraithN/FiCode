// provider 模块：封装与 AI Provider 相关的功能

pub mod base_client;
pub mod client;
pub mod provider;

// 重新导出常用类型，简化外部使用
pub use base_client::{
    extract_text, send_with_retry, AIClient, ApiResponse, Chunk, ChunkContent, FinishReason,
    RetryConfig,
};
pub use client::{AnthropicClient, OpenAiClient};
pub use provider::Provider;

// 从 tools 模块重新导出工具调用函数
#[allow(unused_imports)]
pub use crate::tools::{execute_tool_calls, tool_call};
