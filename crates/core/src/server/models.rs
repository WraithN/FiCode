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

use serde::{Deserialize, Serialize};

// 已从 fi-code-shared crate 重新导出，保留此 re-export 维持向后兼容
pub use fi_code_shared::dto::{
    ApiResponse, CreateSessionRequest, RenameSessionRequest, SessionDto,
};

/// Session 列表响应，非共享类型，保留在 core 中。
#[derive(Debug, Serialize)]
pub struct SessionListResponse {
    pub sessions: Vec<SessionDto>,
    pub current_session_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_response_success() {
        let resp: ApiResponse<i32> = ApiResponse::success(42);
        assert!(resp.success);
        assert_eq!(resp.data, Some(42));
        assert!(resp.error.is_none());
        assert!(resp.code.is_none());

        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"success\":true"));
        assert!(json.contains("\"data\":42"));
        assert!(!json.contains("error"));
        assert!(!json.contains("code"));
    }

    #[test]
    fn test_api_response_error() {
        let resp: ApiResponse<i32> = ApiResponse::error("not found", "ERR_404");
        assert!(!resp.success);
        assert!(resp.data.is_none());
        assert_eq!(resp.error, Some("not found".to_string()));
        assert_eq!(resp.code, Some("ERR_404".to_string()));

        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"success\":false"));
        assert!(json.contains("\"error\":\"not found\""));
        assert!(json.contains("\"code\":\"ERR_404\""));
        assert!(!json.contains("data"));
    }

    #[test]
    fn test_session_dto_serde() {
        let dto = SessionDto {
            id: "sess_001".to_string(),
            name: "Test Session".to_string(),
            created_at: "2025-05-14T10:00:00Z".to_string(),
            last_active: "2025-05-14T12:00:00Z".to_string(),
            message_count: 42,
            is_current: true,
        };

        let json = serde_json::to_string(&dto).unwrap();
        assert!(json.contains("\"id\":\"sess_001\""));
        assert!(json.contains("\"name\":\"Test Session\""));
        assert!(json.contains("\"message_count\":42"));
        assert!(json.contains("\"is_current\":true"));
    }
}
