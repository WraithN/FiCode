use anyhow::Result;
use async_trait::async_trait;

use super::types::{CallToolResult, InitializeResult, ListToolsResult};

// =============================================================================
// McpClient Trait
// =============================================================================
// 所有 MCP 客户端（stdio / HTTP）都必须实现此 trait。
// `Send + Sync` 确保客户端可以安全地跨线程共享。

#[async_trait]
pub trait McpClient: Send + Sync {
    /// 初始化握手。必须在首次使用客户端前调用。
    async fn initialize(&mut self) -> Result<InitializeResult>;

    /// 获取服务器提供的所有工具列表。
    async fn list_tools(&self) -> Result<ListToolsResult>;

    /// 调用指定工具。
    async fn call_tool(&self, name: &str, arguments: serde_json::Value) -> Result<CallToolResult>;
}
