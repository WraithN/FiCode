// =============================================================================
// agent 模块：封装与 AI Agent 交互相关的核心类型与逻辑
// =============================================================================

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::provider::base_client::{AIClient, ChunkContent, FinishReason};
use crate::provider::execute_tool_calls;
use crate::tools::tool_schema;

// =============================================================================
// Rust 结构体和枚举定义
// =============================================================================

/// 对话角色枚举
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
    System,
    Developer,
}

/// 内容块枚举：模型返回的消息可能由多个文本块、思考块或工具调用块组成。
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Part {
    Text { text: String },
    Image { source: ImageSource },
    ToolUse {
        id: String,
        name: String,
        arguments: serde_json::Value,
    },
    ToolResult {
        tool_call_id: String,
        content: String,
        is_error: bool,
    },
    Reasoning {
        thinking: String,
        signature: Option<String>,
    },
}

/// 图片来源枚举
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ImageSource {
    Path { path: String },
    Base64 { media_type: String, data: String },
    Url { url: String },
}

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

fn current_timestamp_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// 对话循环状态，保存消息历史、当前轮数以及状态迁移原因。
#[derive(Debug)]
pub struct LoopState {
    pub messages: Vec<Message>,
    pub turn_count: usize,
    pub transition_reason: Option<String>,
}

impl LoopState {
    pub fn new(messages: Vec<Message>) -> Self {
        Self {
            messages,
            turn_count: 1,
            transition_reason: None,
        }
    }
}

/// 系统级 Prompt，定义 Agent 的行为准则。
pub const SYSTEM_PROMPT: &str = "You are a coding agent. \
    Use bash to inspect and change the workspace. Act first, then report clearly.";

// =============================================================================
// 异步函数：运行一轮对话
// =============================================================================

/// 运行单轮对话：
/// 1. 通过 `stream_message` 发起流式请求，并传入闭包实时消费 Chunk；
/// 2. 在闭包内部将同类型的文本/思考增量聚合为完整的内容块；
/// 3. tool_use 由客户端拼装完整后，以 `ChunkContent::ToolUse` 形式传入闭包；
/// 4. 将 assistant 回复追加到状态；
/// 5. 若停止原因为 `ToolUse`，则执行工具调用并将结果以 user 身份回传；
/// 6. 返回 `true` 表示需要继续下一轮，`false` 表示本轮结束。
pub async fn run_one_turn<C: AIClient + ?Sized>(client: &C, state: &mut LoopState) -> Result<bool> {
    let mut content_blocks = Vec::new();
    let mut finish_reason = None;

    client
        .stream_message(
            SYSTEM_PROMPT,
            &state.messages,
            &tool_schema(),
            &mut |chunk| {
                match chunk.content {
                    // 文本增量：与最后一个 Text 块合并，避免历史记录碎片化
                    ChunkContent::Text(text) => {
                        if let Some(Part::Text { text: last }) = content_blocks.last_mut() {
                            last.push_str(&text);
                        } else {
                            content_blocks.push(Part::Text { text });
                        }
                    }
                    // 思考增量：与最后一个 Reasoning 块合并
                    ChunkContent::Think(text) => {
                        if let Some(Part::Reasoning { thinking: last, .. }) = content_blocks.last_mut() {
                            last.push_str(&text);
                        } else {
                            content_blocks.push(Part::Reasoning { thinking: text, signature: None });
                        }
                    }
                    // 完整的工具调用块（客户端已拼装完毕）
                    ChunkContent::ToolUse(tool) => {
                        content_blocks.push(tool);
                    }
                    // 流结束标志
                    ChunkContent::Finish(reason) => {
                        finish_reason = Some(reason);
                    }
                }
            },
        )
        .await?;

    let session_id = state
        .messages
        .last()
        .map(|m| m.session_id.clone())
        .unwrap_or_default();

    state.messages.push(Message::new(
        session_id.clone(),
        Role::Assistant,
        content_blocks.clone(),
    ));

    // 判断停止原因：只有明确为 ToolUse 时才继续执行工具调用回合
    if finish_reason != Some(FinishReason::ToolUse) {
        state.transition_reason = None;
        return Ok(false);
    }

    let tool_results = execute_tool_calls(&content_blocks);
    if tool_results.is_empty() {
        state.transition_reason = None;
        return Ok(false);
    }

    state.messages.push(Message::new(
        session_id,
        Role::User,
        tool_results,
    ));

    state.turn_count += 1;
    state.transition_reason = Some("tool_result".to_string());

    Ok(true)
}

// =============================================================================
// 异步函数：代理主循环
// =============================================================================

/// Agent 主循环：不断调用 `run_one_turn` 直到对话自然结束。
pub async fn agent_loop<C: AIClient + ?Sized>(client: &C, state: &mut LoopState) -> Result<()> {
    while run_one_turn(client, state).await? {}
    Ok(())
}
