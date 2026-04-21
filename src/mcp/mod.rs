// MCP (Model Context Protocol) 模块
// 提供本地 (stdio) 和远程 (HTTP) MCP 服务器的连接、工具发现与调用能力

pub mod client;
pub mod manager;
pub mod transport;
pub mod types;

pub use client::McpClient;
pub use manager::{McpManager, McpServerStatus};
pub use types::*;
