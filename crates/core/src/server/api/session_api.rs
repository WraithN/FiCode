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
    extract::{Path, State},
    http::StatusCode,
    Json,
};

use crate::log_error;
use crate::server::models::{
    ApiResponse, CreateSessionRequest, RenameSessionRequest, SessionDto, SessionListResponse,
};
use crate::server::server::AppState;

/// 将毫秒时间戳转换为 RFC3339 字符串
fn ms_to_rfc3339(ms: u64) -> String {
    chrono::DateTime::from_timestamp_millis(ms as i64)
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_else(|| chrono::Utc::now().to_rfc3339())
}

pub async fn list_sessions(
    State(state): State<AppState>,
) -> Json<ApiResponse<SessionListResponse>> {
    let sessions = if let Some(ref sm) = state.session_manager {
        match sm.list_sessions() {
            Ok(metas) => metas
                .into_iter()
                .map(|m| SessionDto {
                    id: m.id.clone(),
                    name: m.id.clone(), // 暂无独立名称，使用 id 作为名称
                    created_at: ms_to_rfc3339(m.created_at),
                    last_active: ms_to_rfc3339(m.updated_at),
                    message_count: m.message_count,
                    is_current: false,
                })
                .collect(),
            Err(e) => {
                log_error!("[Server] list_sessions error: {}", e);
                vec![]
            }
        }
    } else {
        vec![]
    };

    let response = SessionListResponse {
        sessions,
        current_session_id: None,
    };

    Json(ApiResponse::success(response))
}

pub async fn create_session(
    State(state): State<AppState>,
    Json(req): Json<CreateSessionRequest>,
) -> Json<ApiResponse<SessionDto>> {
    let session_id = ulid::Ulid::new().to_string();

    // 在 HttpSessionManager 中创建会话状态
    let loop_state = crate::agent::LoopState::new(Vec::new());
    state.sessions.save(&session_id, loop_state);

    let now = chrono::Utc::now().to_rfc3339();
    let session = SessionDto {
        id: session_id,
        name: req.name,
        created_at: now.clone(),
        last_active: now,
        message_count: 0,
        is_current: true,
    };

    Json(ApiResponse::success(session))
}

pub async fn rename_session(
    State(_state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<RenameSessionRequest>,
) -> Json<ApiResponse<SessionDto>> {
    let session = SessionDto {
        id,
        name: req.name,
        created_at: chrono::Utc::now().to_rfc3339(),
        last_active: chrono::Utc::now().to_rfc3339(),
        message_count: 0,
        is_current: false,
    };

    Json(ApiResponse::success(session))
}

pub async fn delete_session(State(_state): State<AppState>, Path(_id): Path<String>) -> StatusCode {
    StatusCode::NO_CONTENT
}

pub async fn get_session_messages(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<ApiResponse<Vec<crate::session::message::Message>>> {
    let messages = if let Some(ref sm) = state.session_manager {
        match sm.load_session(&id) {
            Ok(session) => session.messages,
            Err(e) => {
                log_error!("[Server] Failed to load session {}: {}", id, e);
                vec![]
            }
        }
    } else {
        vec![]
    };

    Json(ApiResponse::success(messages))
}

pub async fn switch_session(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<ApiResponse<SessionDto>> {
    // 从磁盘加载会话消息到内存
    let message_count = if let Some(ref sm) = state.session_manager {
        match sm.load_session(&id) {
            Ok(session) => {
                let count = session.messages.len();
                let loop_state = crate::agent::LoopState::new(session.messages);
                state.sessions.save(&id, loop_state);
                count
            }
            Err(e) => {
                log_error!("[Server] Failed to load session {}: {}", id, e);
                0
            }
        }
    } else {
        0
    };

    let session = SessionDto {
        id: id.clone(),
        name: id,
        created_at: chrono::Utc::now().to_rfc3339(),
        last_active: chrono::Utc::now().to_rfc3339(),
        message_count,
        is_current: true,
    };

    Json(ApiResponse::success(session))
}
