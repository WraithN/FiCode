use anyhow::{Context, Result};
use bytes::Bytes;
use futures::Stream;
use futures::StreamExt;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use serde_json::json;
use std::collections::HashMap;

use crate::agent::{ContentBlock, Message};
use crate::provider::base_client::{AIClient, Chunk, ChunkContent, FinishReason, RetryConfig, send_with_retry};

/// Anthropic API 客户端。
/// 内部仅持有 HTTP 客户端与统一的 `Model` 配置。
pub struct AnthropicClient {
    client: reqwest::Client,
    model: crate::provider::provider::Model,
    retry_config: RetryConfig,
}

impl AnthropicClient {
    /// 根据 `Model` 配置构造客户端。
    pub fn from_model(model: &crate::provider::provider::Model) -> Result<Self> {
        Ok(Self {
            client: reqwest::Client::new(),
            model: model.clone(),
            retry_config: RetryConfig::default(),
        })
    }
}

#[async_trait::async_trait]
impl AIClient for AnthropicClient {
    async fn stream_message(
        &self,
        system_prompt: &str,
        messages: &[Message],
        tools_schema: &serde_json::Value,
        on_chunk: &mut (dyn FnMut(Chunk) + Send),
    ) -> Result<()> {
        // 构造请求头
        let mut headers = HeaderMap::new();
        headers.insert("x-api-key", HeaderValue::from_str(&self.model.api_key)?);
        headers.insert("anthropic-version", HeaderValue::from_static("2025-06-01"));
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        // 构造请求体，显式开启流式模式
        let body = json!({
            "model": self.model.model_name,
            "system": system_prompt,
            "messages": messages,
            "tools": *tools_schema,
            "max_tokens": 8000,
            "stream": true
        });

        let url = format!("{}/v1/messages", self.model.base_url);
        let request = self
            .client
            .post(&url)
            .headers(headers)
            .json(&body)
            .build()?;
        let resp = send_with_retry(&self.client, request, &self.retry_config).await?;

        // 检查 HTTP 状态码
        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Anthropic API error ({}): {}", status, text));
        }

        // 直接读取 SSE 字节流并调用闭包
        let byte_stream = resp.bytes_stream();
        parse_anthropic_sse(byte_stream, on_chunk).await
    }
}

// =============================================================================
// Anthropic SSE 解析：将原生 Server-Sent Events 通过闭包实时回传
// =============================================================================

/// 解析 Anthropic 的 SSE 字节流，并在解析过程中直接调用 `on_chunk`。
///
/// 关键事件映射：
/// - `content_block_delta` + `text_delta`       => `ChunkContent::Text`
/// - `content_block_delta` + `thinking_delta`   => `ChunkContent::Think`
/// - `content_block_start` + `tool_use`         => 记录工具调用元数据
/// - `content_block_delta` + `input_json_delta` => 累积工具参数 JSON
/// - `content_block_stop`                       => 拼装完整 ToolUse 并回传
/// - `message_delta` 中的 `stop_reason`         => `ChunkContent::Finish`
async fn parse_anthropic_sse<S>(
    byte_stream: S,
    on_chunk: &mut (dyn FnMut(Chunk) + Send),
) -> Result<()>
where
    S: Stream<Item = std::result::Result<Bytes, reqwest::Error>> + Send + 'static,
{
    let mut buffer = String::new();
    // 维护 index -> (tool_id, tool_name, args_json_string)
    let mut index_to_tool: HashMap<usize, (String, String, String)> = HashMap::new();
    let mut current_event_type: Option<String> = None;

    tokio::pin!(byte_stream);
    while let Some(chunk) = byte_stream.next().await {
        let chunk = chunk?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        // 按行处理 SSE 数据
        while let Some(pos) = buffer.find('\n') {
            let line = buffer.drain(..=pos).collect::<String>();
            let line = line.trim_end();

            if line.starts_with("event:") {
                current_event_type = Some(line[6..].trim().to_string());
            } else if line.starts_with("data:") {
                let data = line[5..].trim();
                if data == "[DONE]" {
                    continue;
                }

                let event_type = current_event_type.take().unwrap_or_default();
                let json: serde_json::Value = serde_json::from_str(data)
                    .with_context(|| format!("Failed to parse Anthropic SSE data: {}", data))?;

                match event_type.as_str() {
                    "content_block_start" => {
                        if let Some(block) = json.get("content_block") {
                            let block_type = block.get("type").and_then(|v| v.as_str()).unwrap_or("");
                            if block_type == "tool_use" {
                                let index = json.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                                let id = block.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                let name = block.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                index_to_tool.insert(index, (id, name, String::new()));
                            }
                        }
                    }
                    "content_block_delta" => {
                        if let Some(delta) = json.get("delta") {
                            let delta_type = delta.get("type").and_then(|v| v.as_str()).unwrap_or("");
                            match delta_type {
                                "text_delta" => {
                                    if let Some(text) = delta.get("text").and_then(|v| v.as_str()) {
                                        on_chunk(Chunk {
                                            content: ChunkContent::Text(text.to_string()),
                                        });
                                    }
                                }
                                "thinking_delta" => {
                                    if let Some(text) = delta.get("thinking").and_then(|v| v.as_str()) {
                                        on_chunk(Chunk {
                                            content: ChunkContent::Think(text.to_string()),
                                        });
                                    }
                                }
                                "input_json_delta" => {
                                    let index = json.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                                    if let Some((_, _, args)) = index_to_tool.get_mut(&index) {
                                        if let Some(partial) = delta.get("partial_json").and_then(|v| v.as_str()) {
                                            args.push_str(partial);
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    "content_block_stop" => {
                        let index = json.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                        if let Some((id, name, args)) = index_to_tool.remove(&index) {
                            let input: HashMap<String, serde_json::Value> =
                                serde_json::from_str(&args).unwrap_or_default();
                            on_chunk(Chunk {
                                content: ChunkContent::ToolUse(ContentBlock::ToolUse { id, name, input }),
                            });
                        }
                    }
                    "message_delta" => {
                        if let Some(stop) = json
                            .get("delta")
                            .and_then(|d| d.get("stop_reason"))
                            .and_then(|v| v.as_str())
                        {
                            on_chunk(Chunk {
                                content: ChunkContent::Finish(FinishReason::from_anthropic(stop)),
                            });
                        }
                    }
                    _ => {}
                }
            } else if line.is_empty() {
                // SSE 空行表示一个事件结束，重置 event 类型
                current_event_type = None;
            }
        }
    }

    Ok(())
}
