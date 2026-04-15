use anyhow::{Context, Result};
use bytes::Bytes;
use futures::Stream;
use futures::StreamExt;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::Serialize;
use serde_json::json;
use std::collections::HashMap;

use crate::agent::{ContentBlock, Message};
use crate::provider::base_client::{AIClient, Chunk, ChunkContent, FinishReason, RetryConfig, send_with_retry};

// =============================================================================
// OpenAI API 兼容客户端
// =============================================================================

pub struct OpenAiClient {
    client: reqwest::Client,
    model: crate::provider::provider::Model,
    retry_config: RetryConfig,
}

impl OpenAiClient {
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
impl AIClient for OpenAiClient {
    async fn stream_message(
        &self,
        system_prompt: &str,
        messages: &[Message],
        tools_schema: &serde_json::Value,
        on_chunk: &mut (dyn FnMut(Chunk) + Send),
    ) -> Result<()> {
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", self.model.api_key))?,
        );
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let openai_messages = build_messages(system_prompt, messages);

        // 显式开启流式模式
        let body = json!({
            "model": self.model.model_name,
            "messages": openai_messages,
            "tools": convert_tools_schema(tools_schema),
            "max_tokens": 8000,
            "stream": true
        });

        let url = format!("{}/v1/chat/completions", self.model.base_url);
        let request = self
            .client
            .post(&url)
            .headers(headers)
            .json(&body)
            .build()?;
        let resp = send_with_retry(&self.client, request, &self.retry_config).await?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("OpenAI API error ({}): {}", status, text));
        }

        let byte_stream = resp.bytes_stream();
        parse_openai_sse(byte_stream, on_chunk).await
    }
}

// =============================================================================
// OpenAI SSE 解析：将原生 Server-Sent Events 通过闭包实时回传
// =============================================================================

/// 解析 OpenAI 的 SSE 字节流，并在解析过程中直接调用 `on_chunk`。
///
/// 解析逻辑：
/// - `delta.content` 存在 => 直接回传 `ChunkContent::Text`
/// - `delta.tool_calls` 存在 => 在内存中累积每个 index 的 (id, name, arguments)
/// - `finish_reason` 存在 => 若因 tool_calls 结束，先将所有拼好的 tool_use 回传，
///   最后统一回传 `ChunkContent::Finish`
async fn parse_openai_sse<S>(
    byte_stream: S,
    on_chunk: &mut (dyn FnMut(Chunk) + Send),
) -> Result<()>
where
    S: Stream<Item = std::result::Result<Bytes, reqwest::Error>> + Send + 'static,
{
    let mut buffer = String::new();
    // OpenAI 的 tool_calls 增量只带 `index`，需要维护 index -> (id, name, args_buffer)
    let mut index_to_tool: HashMap<usize, (Option<String>, Option<String>, String)> = HashMap::new();

    tokio::pin!(byte_stream);
    while let Some(chunk) = byte_stream.next().await {
        let chunk = chunk?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(pos) = buffer.find('\n') {
            let line = buffer.drain(..=pos).collect::<String>();
            let line = line.trim_end();

            if line.starts_with("data:") {
                let data = line[5..].trim();
                if data == "[DONE]" {
                    continue;
                }

                let json: serde_json::Value = serde_json::from_str(data)
                    .with_context(|| format!("Failed to parse OpenAI SSE data: {}", data))?;

                // OpenAI 的 choices 数组通常只有一个元素
                if let Some(choices) = json.get("choices").and_then(|v| v.as_array()) {
                    for choice in choices {
                        let finish_reason = choice.get("finish_reason").and_then(|v| v.as_str());

                        if let Some(delta) = choice.get("delta") {
                            // 文本增量：直接回传
                            if let Some(text) = delta.get("content").and_then(|v| v.as_str()) {
                                if !text.is_empty() {
                                    on_chunk(Chunk {
                                        content: ChunkContent::Text(text.to_string()),
                                    });
                                }
                            }

                            // 工具调用增量：仅更新内存状态，暂不回传
                            if let Some(tools) = delta.get("tool_calls").and_then(|v| v.as_array()) {
                                for tool in tools {
                                    let index = tool.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                                    let id = tool.get("id").and_then(|v| v.as_str()).map(|s| s.to_string());
                                    let name = tool
                                        .get("function")
                                        .and_then(|f| f.get("name"))
                                        .and_then(|v| v.as_str())
                                        .map(|s| s.to_string());
                                    let args = tool
                                        .get("function")
                                        .and_then(|f| f.get("arguments"))
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("")
                                        .to_string();

                                    let entry = index_to_tool.entry(index).or_insert((None, None, String::new()));
                                    if let Some(id) = id {
                                        entry.0 = Some(id);
                                    }
                                    if let Some(name) = name {
                                        entry.1 = Some(name);
                                    }
                                    entry.2.push_str(&args);
                                }
                            }
                        }

                        // 当收到 finish_reason 时，说明所有增量已结束
                        if let Some(finish) = finish_reason {
                            // 若因工具调用结束，先将拼好的完整 tool_use 回传
                            if finish == "tool_calls" {
                                let mut indices: Vec<usize> = index_to_tool.keys().cloned().collect();
                                indices.sort();
                                for idx in indices {
                                    if let Some((Some(id), Some(name), args)) = index_to_tool.remove(&idx) {
                                        let input: HashMap<String, serde_json::Value> =
                                            serde_json::from_str(&args).unwrap_or_default();
                                        on_chunk(Chunk {
                                            content: ChunkContent::ToolUse(ContentBlock::ToolUse {
                                                id,
                                                name,
                                                input,
                                            }),
                                        });
                                    }
                                }
                            }

                            on_chunk(Chunk {
                                content: ChunkContent::Finish(FinishReason::from_openai(finish)),
                            });
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

// =============================================================================
// 请求/响应结构体（仅用于序列化请求体）
// =============================================================================

#[derive(Debug, Serialize)]
struct OpenAiMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OpenAiToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct OpenAiToolCall {
    id: String,
    #[serde(rename = "type")]
    call_type: String,
    function: OpenAiFunctionCall,
}

#[derive(Debug, Serialize)]
struct OpenAiFunctionCall {
    name: String,
    arguments: String,
}

// =============================================================================
// 辅助函数：转换消息格式
// =============================================================================

/// 将内部 `Message` 列表转换为 OpenAI 兼容的消息格式。
/// 注意处理 tool_result 数组以及 assistant 消息中的工具调用片段。
fn build_messages(system_prompt: &str, messages: &[Message]) -> Vec<OpenAiMessage> {
    let mut result = Vec::new();

    result.push(OpenAiMessage {
        role: "system".to_string(),
        content: Some(system_prompt.to_string()),
        tool_calls: None,
        tool_call_id: None,
    });

    for msg in messages {
        match msg.role.as_str() {
            "user" => {
                if let Some(content) = &msg.content {
                    if let Some(arr) = content.as_array() {
                        for item in arr {
                            if let Some(tool_use_id) = item.get("tool_use_id").and_then(|v| v.as_str()) {
                                let text = item.get("content")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                result.push(OpenAiMessage {
                                    role: "tool".to_string(),
                                    content: Some(text),
                                    tool_calls: None,
                                    tool_call_id: Some(tool_use_id.to_string()),
                                });
                            } else {
                                let text = item.get("text")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                if !text.is_empty() {
                                    result.push(OpenAiMessage {
                                        role: "user".to_string(),
                                        content: Some(text),
                                        tool_calls: None,
                                        tool_call_id: None,
                                    });
                                }
                            }
                        }
                    } else if let Some(s) = content.as_str() {
                        result.push(OpenAiMessage {
                            role: "user".to_string(),
                            content: Some(s.to_string()),
                            tool_calls: None,
                            tool_call_id: None,
                        });
                    } else {
                        result.push(OpenAiMessage {
                            role: "user".to_string(),
                            content: Some(content.to_string()),
                            tool_calls: None,
                            tool_call_id: None,
                        });
                    }
                }
            }
            "assistant" => {
                let mut text_parts = Vec::new();
                let mut tool_calls = Vec::new();

                if let Some(content) = &msg.content {
                    if let Some(arr) = content.as_array() {
                        for item in arr {
                            if let Some(t) = item.get("text").and_then(|v| v.as_str()) {
                                text_parts.push(t.to_string());
                            } else if let Some(name) = item.get("name").and_then(|v| v.as_str()) {
                                let id = item.get("id").and_then(|v| v.as_str()).unwrap_or("call_unknown").to_string();
                                let input = item.get("input").cloned().unwrap_or(json!({}));
                                tool_calls.push(OpenAiToolCall {
                                    id,
                                    call_type: "function".to_string(),
                                    function: OpenAiFunctionCall {
                                        name: name.to_string(),
                                        arguments: input.to_string(),
                                    },
                                });
                            }
                        }
                    } else if let Some(s) = content.as_str() {
                        text_parts.push(s.to_string());
                    } else {
                        text_parts.push(content.to_string());
                    }
                }

                let content_text = if text_parts.is_empty() {
                    None
                } else {
                    Some(text_parts.join("\n"))
                };

                result.push(OpenAiMessage {
                    role: "assistant".to_string(),
                    content: content_text,
                    tool_calls: if tool_calls.is_empty() { None } else { Some(tool_calls) },
                    tool_call_id: None,
                });
            }
            _ => {}
        }
    }

    result
}

// =============================================================================
// 辅助函数：转换工具 schema 和响应
// =============================================================================

/// 将内部工具注册表生成的 schema 转换为 OpenAI 要求的 `tools` 格式。
fn convert_tools_schema(tools_schema: &serde_json::Value) -> serde_json::Value {
    if let Some(arr) = tools_schema.as_array() {
        let converted: Vec<serde_json::Value> = arr
            .iter()
            .map(|tool| {
                let name = tool.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let description = tool.get("description").and_then(|v| v.as_str()).unwrap_or("");
                let parameters = tool.get("input_schema").cloned().unwrap_or(json!({}));
                json!({
                    "type": "function",
                    "function": {
                        "name": name,
                        "description": description,
                        "parameters": parameters
                    }
                })
            })
            .collect();
        serde_json::Value::Array(converted)
    } else {
        json!([])
    }
}
