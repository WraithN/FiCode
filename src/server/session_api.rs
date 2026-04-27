use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

use crate::server::models::{
    ApiResponse, CreateSessionRequest, RenameSessionRequest, SessionDto, SessionListResponse,
};
use crate::server::server::AppState;

pub async fn list_sessions(
    State(_state): State<AppState>,
) -> Json<ApiResponse<SessionListResponse>> {
    let sessions = vec![];

    let response = SessionListResponse {
        sessions,
        current_session_id: None,
    };

    Json(ApiResponse::success(response))
}

pub async fn create_session(
    State(_state): State<AppState>,
    Json(req): Json<CreateSessionRequest>,
) -> Json<ApiResponse<SessionDto>> {
    let session = SessionDto {
        id: ulid::Ulid::new().to_string(),
        name: req.name,
        created_at: chrono::Utc::now().to_rfc3339(),
        last_active: chrono::Utc::now().to_rfc3339(),
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

pub async fn switch_session(
    State(_state): State<AppState>,
    Path(id): Path<String>,
) -> Json<ApiResponse<SessionDto>> {
    let session = SessionDto {
        id,
        name: "switched".to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
        last_active: chrono::Utc::now().to_rfc3339(),
        message_count: 0,
        is_current: true,
    };

    Json(ApiResponse::success(session))
}
