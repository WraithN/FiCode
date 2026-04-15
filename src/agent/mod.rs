// =============================================================================
// agent 模块：封装与 AI Agent 交互相关的核心类型与逻辑
// =============================================================================
// 本模块定义了对话中使用的核心数据结构：
// - `Role`：消息发送者角色（User/Assistant/System/Developer）
// - `Part`：单条消息的内容块，支持文本、图片、工具调用、工具结果、推理过程
// - `Message`：包含元数据（id、session_id、时间戳等）的完整消息
// - `LoopState`：agent 循环的运行时状态
//
// 设计演进：此前使用简单的 `content: Option<serde_json::Value>` 和 `ContentBlock`，
// 为了支持 Session 持久化与多模态内容，现统一升级为强类型的 `Message` / `Part` 模型。

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::provider::base_client::{AIClient, ChunkContent, FinishReason};
use crate::provider::execute_tool_calls;
use crate::tools::tool_schema;

// =============================================================================
// 角色枚举
// =============================================================================

/// 对话角色枚举。
/// - `User`：人类用户
/// - `Assistant`：AI 助手
/// - `System`：系统级提示（如环境描述）
/// - `Developer`：开发者消息（部分模型支持，如 Claude Code 风格）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
    System,
    Developer,
}

// =============================================================================
// 内容块枚举（Part）：消息的原子组成单元
// =============================================================================

/// 内容块枚举：一条 `Message` 由多个 `Part` 按顺序组成。
///
/// 这种设计与 Anthropic / OpenAI 的最新内容块 API 对齐，
/// 支持纯文本、多模态图片、工具调用、工具结果以及推理过程。
///
/// `#[serde(tag = "type", rename_all = "snake_case")]` 保证序列化/反序列化时
/// 使用 `"type"` 字段做分支，且字段名为 snake_case。
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
        arguments: serde_json::Value,
    },
    /// 工具执行结果（由 User 角色消息携带，回传给模型）
    ToolResult {
        tool_call_id: String,
        content: String,
        is_error: bool,
    },
    /// 推理/思考过程（如 Claude Extended Thinking）
    Reasoning {
        thinking: String,
        /// 可选的签名，用于验证推理内容未被篡改
        signature: Option<String>,
    },
}

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

// =============================================================================
// 消息结构体（Message）
// =============================================================================

/// 对话消息结构体，用于在多轮对话中保存角色与内容块。
///
/// 相比旧设计，新增了以下持久化与追踪字段：
/// - `id`：ULID 生成的全局唯一标识
/// - `session_id`：所属会话的外键
/// - `role`：强类型的 Role 枚举
/// - `created_at`：Unix 时间戳（毫秒）
/// - `parts`：Vec<Part> 替代了原来的 `Option<serde_json::Value>`
/// - `token_count` / `cost`：可选的用量统计
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
fn current_timestamp_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

// =============================================================================
// 对话循环状态（LoopState）
// =============================================================================

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

    // 从当前消息历史中继承 session_id，确保工具结果消息与对话属于同一会话
    let session_id = state
        .messages
        .last()
        .map(|m| m.session_id.clone())
        .unwrap_or_default();

    // 将 Assistant 的完整回复追加到状态
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

    // 执行所有工具调用，并收集结果
    let tool_results = execute_tool_calls(&content_blocks);
    if tool_results.is_empty() {
        state.transition_reason = None;
        return Ok(false);
    }

    // 将工具结果封装为 User 消息回传（符合 OpenAI / Anthropic API 的角色交替要求）
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
