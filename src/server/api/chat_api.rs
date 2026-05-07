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

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;
use tokio_stream::StreamExt;

use crate::agent::{agent_loop, LoopState};
use crate::session::message::{Message, Part, Role};
use crate::tools::set_task_provider;

use super::super::server::{check_auth, AppState};
use super::super::transport::rpc::JsonRpcResponse;
use super::super::transport::sse::{create_sse_channel, SseEvent, SseSender};

/// Chat 请求体
#[derive(Deserialize)]
pub struct ChatRequest {
    pub session_id: Option<String>,
    pub message: String,
}

/// Chat 端点处理器 — 返回 SSE
pub async fn handle_chat_endpoint(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<ChatRequest>,
) -> Response {
    // 认证检查
    if let Some(resp) = check_auth(&headers, &state.config).await {
        return Json(resp).into_response();
    }

    let session_id = match req.session_id {
        Some(id) => {
            if state.sessions.get(&id).is_none() {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(JsonRpcResponse::error(
                        -32001,
                        "Session not found",
                        Some(Value::Null),
                    )),
                )
                    .into_response();
            }
            id
        }
        None => state.sessions.create(),
    };

    let (sse_sender, sse_stream) = create_sse_channel(128);

    // 在后台 task 中运行 agent_chat
    tokio::spawn(run_agent_chat(
        state,
        session_id.clone(),
        req.message,
        sse_sender,
    ));

    // 返回 SSE 响应
    let stream = sse_stream.map(|event| {
        let data = serde_json::to_string(&event).unwrap_or_default();
        Ok::<_, std::convert::Infallible>(axum::response::sse::Event::default().data(data))
    });
    axum::response::Sse::new(stream).into_response()
}

async fn send_last_assistant_text(messages: &[Message], sse_sender: &SseSender) {
    let Some(last_msg) = messages.last() else { return };
    if last_msg.role != Role::Assistant {
        return;
    }
    let text = last_msg
        .parts
        .iter()
        .filter_map(|p| match p {
            Part::Text { text } => Some(text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("");
    if !text.is_empty() {
        let _ = sse_sender.send(SseEvent::Message { content: text }).await;
    }

    // 发送结构化详情（思考过程、工具调用等）
    let blocks: Vec<crate::server::transport::sse::DetailBlock> = last_msg
        .parts
        .iter()
        .filter_map(|p| match p {
            Part::Reasoning { thinking, .. } => {
                Some(crate::server::transport::sse::DetailBlock::Reasoning {
                    thinking: thinking.clone(),
                })
            }
            Part::ToolUse { id, name, arguments } => {
                let args_str = serde_json::to_string_pretty(arguments).unwrap_or_default();
                Some(crate::server::transport::sse::DetailBlock::ToolUse {
                    id: id.clone(),
                    name: name.clone(),
                    arguments: args_str,
                })
            }
            Part::ToolResult {
                tool_call_id,
                content,
                is_error,
            } => Some(crate::server::transport::sse::DetailBlock::ToolResult {
                tool_use_id: tool_call_id.clone(),
                content: content.clone(),
                is_error: *is_error,
            }),
            _ => None,
        })
        .collect();

    if !blocks.is_empty() {
        let _ = sse_sender
            .send(SseEvent::MessageDetails { blocks })
            .await;
    }
}

/// 后台运行 Agent 对话
async fn run_agent_chat(
    state: AppState,
    session_id: String,
    message: String,
    sse_sender: SseSender,
) {
    // 设置全局 Provider，供 handle_task_plan 工具使用
    set_task_provider(Arc::clone(&state.provider));

    // 获取或创建 LoopState
    let mut loop_state = match state.sessions.get(&session_id) {
        Some(state) => state,
        None => {
            let _ = sse_sender
                .send(SseEvent::Error {
                    message: "Session not found".to_string(),
                })
                .await;
            return;
        }
    };

    // 添加用户消息
    let user_msg = Message::new(
        session_id.clone(),
        Role::User,
        vec![Part::Text { text: message }],
    );
    loop_state.messages.push(user_msg);

    // 获取客户端（先读取并释放锁，避免 guard 跨越 await）
    let client_result = match state.provider.read() {
        Ok(p) => p
            .get_client()
            .map_err(|e| format!("Failed to create client: {}", e)),
        Err(_) => Err("Provider lock poisoned".to_string()),
    };
    let client = match client_result {
        Ok(c) => c,
        Err(msg) => {
            let _ = sse_sender.send(SseEvent::Error { message: msg }).await;
            return;
        }
    };

    // 运行 agent_loop
    if let Err(e) = agent_loop(client.as_ref(), &mut loop_state).await {
        let _ = sse_sender
            .send(SseEvent::Error {
                message: format!("Agent loop error: {}", e),
            })
            .await;
    } else {
        // 发送 assistant 的最后回复
        send_last_assistant_text(&loop_state.messages, &sse_sender).await;
    }

    // 保存会话状态
    state.sessions.save(&session_id, loop_state);

    // 发送 done 事件
    let _ = sse_sender.send(SseEvent::Done { session_id }).await;
}
