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
    extract::{Query, State},
    response::{sse::Event, Sse},
    Json,
};
use serde::Deserialize;
use std::convert::Infallible;
use tokio_stream::StreamExt;

use crate::server::models::ApiResponse;
use crate::server::server::AppState;
use crate::utils::log_store::LogEntry;

#[derive(Deserialize)]
pub struct ListLogsQuery {
    limit: Option<usize>,
}

/// 列出最近的日志条目
pub async fn handle_list_logs(
    State(state): State<AppState>,
    Query(query): Query<ListLogsQuery>,
) -> Json<ApiResponse<Vec<LogEntry>>> {
    let limit = query.limit.unwrap_or(200).min(1000);
    let logs = match &state.log_broadcaster {
        Some(b) => b.recent(limit),
        None => Vec::new(),
    };
    Json(ApiResponse::success(logs))
}

/// SSE 流式推送日志
pub async fn handle_log_stream(
    State(state): State<AppState>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let rx = match &state.log_broadcaster {
        Some(b) => b.subscribe(),
        None => {
            let (tx, rx) = tokio::sync::broadcast::channel(1);
            drop(tx);
            rx
        }
    };

    let stream = tokio_stream::wrappers::BroadcastStream::new(rx)
        .filter_map(|result| {
            match result {
                Ok(entry) => {
                    let data = serde_json::to_string(&entry).unwrap_or_default();
                    Some(Ok::<_, Infallible>(Event::default().data(data)))
                }
                Err(_) => None,
            }
        });

    Sse::new(stream)
}
