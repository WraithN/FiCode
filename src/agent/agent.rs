// =============================================================================
// agent 模块：封装与 AI Agent 交互相关的核心类型与逻辑
// =============================================================================
// 本模块定义了对话中使用的核心数据结构与 agent 循环：
// - `LoopState`：agent 循环的运行时状态
// - `run_one_turn` / `agent_loop`：单轮/多轮对话驱动逻辑
//
// 消息类型（Message / Part / Role / ImageSource）已从本模块迁移到
// 独立的 `message` 模块，供 session、provider、tools 等多个模块共享。

use anyhow::Result;

use crate::agent::PromptBuilder;
use crate::log_block;
use crate::log_debug;
use crate::log_trace;
use crate::provider::base_client::{AIClient, ChunkContent, FinishReason};
use crate::provider::execute_tool_calls;
use crate::session::message::{Message, Part, Role};
use crate::tools::tool_schema;

// =============================================================================
// 对话循环状态（LoopState）
// =============================================================================

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

static PROMPT_LOGGED_ONCE: std::sync::Once = std::sync::Once::new();

// =============================================================================
// 异步函数：运行一轮对话
// =============================================================================

pub async fn run_one_turn<C: AIClient + ?Sized>(client: &C, state: &mut LoopState) -> Result<bool> {
    let mut content_blocks = Vec::new();
    let mut finish_reason = None;

    let system_prompt = PromptBuilder::new().build(&tool_schema());

    PROMPT_LOGGED_ONCE.call_once(|| {
        log_block!(
            crate::utils::log::LogLevel::Debug,
            "SYSTEM PROMPT (first)",
            &system_prompt
        );
    });
    log_block!(
        crate::utils::log::LogLevel::Trace,
        "SYSTEM PROMPT",
        &system_prompt
    );

    log_debug!(
        "run_one_turn start | turn={} | messages={}",
        state.turn_count,
        state.messages.len()
    );

    for (idx, msg) in state.messages.iter().enumerate() {
        let preview: String = msg
            .parts
            .iter()
            .map(|p| format!("{:?}", p))
            .collect::<String>()
            .chars()
            .take(150)
            .collect();
        log_debug!("message[{}] | role={:?} | preview={}", idx, msg.role, preview);
        log_trace!(
            "message[{}] | role={:?} | parts={:?}",
            idx,
            msg.role,
            msg.parts
        );
    }

    log_trace!(
        "tools_schema | {}",
        serde_json::to_string_pretty(&tool_schema()).unwrap_or_default()
    );

    client
        .stream_message(
            &system_prompt,
            &state.messages,
            &tool_schema(),
            &mut |chunk| {
                match chunk.content {
                    ChunkContent::Text(text) => {
                        if let Some(Part::Text { text: last }) = content_blocks.last_mut() {
                            last.push_str(&text);
                        } else {
                            content_blocks.push(Part::Text { text });
                        }
                    }
                    ChunkContent::Think(text) => {
                        if let Some(Part::Reasoning { thinking: last, .. }) =
                            content_blocks.last_mut()
                        {
                            last.push_str(&text);
                        } else {
                            content_blocks.push(Part::Reasoning {
                                thinking: text,
                                signature: None,
                            });
                        }
                    }
                    ChunkContent::ToolUse(ref tool) => {
                        if let Part::ToolUse { id, name, arguments } = tool {
                            log_debug!(
                                "LLM tool_use | id={} | name={} | args={}",
                                id, name, arguments
                            );
                        }
                        content_blocks.push(tool.clone());
                    }
                    ChunkContent::Finish(ref reason) => {
                        log_debug!("LLM finish_reason={:?}", reason);
                        finish_reason = Some(reason.clone());
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

    log_debug!(
        "assistant message appended | blocks={}",
        content_blocks.len()
    );
    for (idx, block) in content_blocks.iter().enumerate() {
        let preview = format!("{:?}", block).chars().take(200).collect::<String>();
        log_debug!("assistant block[{}] | {}", idx, preview);
        log_trace!("assistant block[{}] | {:?}", idx, block);
    }
    state.messages.push(Message::new(
        session_id.clone(),
        Role::Assistant,
        content_blocks.clone(),
    ));

    if finish_reason != Some(FinishReason::ToolUse) {
        state.transition_reason = None;
        log_debug!("run_one_turn end | no tool use");
        return Ok(false);
    }

    let tool_results = execute_tool_calls(&content_blocks);
    if tool_results.is_empty() {
        state.transition_reason = None;
        log_debug!("run_one_turn end | tool_use finish but no results");
        return Ok(false);
    }

    log_debug!(
        "pushing tool_results back to LLM | results={}",
        tool_results.len()
    );
    for (idx, tr) in tool_results.iter().enumerate() {
        log_trace!("tool_result[{}] | {:?}", idx, tr);
    }

    state
        .messages
        .push(Message::new(session_id, Role::User, tool_results));

    state.turn_count += 1;
    state.transition_reason = Some("tool_result".to_string());

    log_debug!("run_one_turn end | will continue next turn");
    Ok(true)
}

// =============================================================================
// 异步函数：代理主循环
// =============================================================================

pub async fn agent_loop<C: AIClient + ?Sized>(client: &C, state: &mut LoopState) -> Result<()> {
    while run_one_turn(client, state).await? {}
    Ok(())
}
