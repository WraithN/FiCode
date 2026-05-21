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

use axum::Json;
use serde::Deserialize;
use serde_json::Value;

use crate::log_debug;

/// 权限确认请求体
#[derive(Deserialize, Debug)]
pub struct PermissionRespondRequest {
    pub tool_call_id: String,
    pub approved: bool,
}

/// 通用 API 响应结构
#[derive(serde::Serialize, Debug)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
    pub error_code: Option<String>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            error_code: None,
        }
    }

    pub fn error(message: impl Into<String>, code: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message.into()),
            error_code: Some(code.into()),
        }
    }
}

/// 处理权限确认响应
/// 前端/TUI 用户在收到 PermissionAsk SSE 事件后，通过此端点回复确认或拒绝
pub async fn handle_permission_respond(
    Json(req): Json<PermissionRespondRequest>,
) -> Json<ApiResponse<Value>> {
    log_debug!(
        "[API] permission/respond | tool_call_id={} | approved={}",
        req.tool_call_id,
        req.approved
    );

    match crate::permission::respond_permission(&req.tool_call_id, req.approved).await {
        Ok(()) => Json(ApiResponse::success(Value::Null)),
        Err(e) => Json(ApiResponse::error(e, "PERMISSION_NOT_FOUND")),
    }
}
