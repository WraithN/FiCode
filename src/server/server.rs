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

use std::sync::{Arc, RwLock};

use anyhow::anyhow;
use axum::{
    extract::State,
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::Deserialize;
use serde_json::Value;
use tokio_stream::StreamExt;
use tower_http::cors::CorsLayer;

use crate::agent::agent_loop;
use crate::commands::registry::CommandRegistry;
use crate::config::Config;
use crate::provider::Provider;

use super::api::file_api;
use super::models::ApiResponse;
use super::transport::rpc::{handle_rpc, JsonRpcRequest, JsonRpcResponse};
use super::session::HttpSessionManager;
use super::api::session_api;
use super::transport::sse::SseEvent;

/// 服务器共享状态
#[derive(Clone)]
pub struct AppState {
    pub provider: Arc<RwLock<Provider>>,
    pub config: Arc<RwLock<Config>>,
    pub sessions: Arc<HttpSessionManager>,
    pub commands: Arc<CommandRegistry>,
    pub themes: Vec<crate::tui::theme::ThemePreset>,
    pub current_theme: Arc<RwLock<String>>,
    pub log_broadcaster: Option<Arc<crate::utils::log_store::LogBroadcaster>>,
}

pub struct Server {
    state: AppState,
    port: u16,
}

impl Server {
    pub fn new(
        provider: Arc<RwLock<Provider>>,
        config: Arc<RwLock<Config>>,
        port_override: Option<u16>,
    ) -> Self {
        let port = port_override
            .or_else(|| {
                let cfg = config.read().ok()?;
                let server_cfg = cfg.server.as_ref()?;
                server_cfg.port
            })
            .unwrap_or(4040);

        let sessions = Arc::new(HttpSessionManager::new());
        let (commands, current_theme) = super::commands::build_command_registry(sessions.clone());

        let themes = crate::tui::theme::ThemePreset::all_presets();

        Self {
            state: AppState {
                provider,
                config,
                sessions,
                commands: Arc::new(commands),
                themes,
                current_theme,
                log_broadcaster: None,
            },
            port,
        }
    }

    pub fn with_log_broadcaster(mut self, broadcaster: Arc<crate::utils::log_store::LogBroadcaster>) -> Self {
        self.state.log_broadcaster = Some(broadcaster);
        self
    }

    pub async fn run(self) {
        let app = Router::new()
            .route("/rpc", post(handle_rpc_endpoint))
            .route("/chat", post(super::api::chat_api::handle_chat_endpoint))
            .route(
                "/api/sessions",
                get(session_api::list_sessions).post(session_api::create_session),
            )
            .route(
                "/api/sessions/:id",
                put(session_api::rename_session).delete(session_api::delete_session),
            )
            .route(
                "/api/sessions/:id/switch",
                post(session_api::switch_session),
            )
            .route("/api/files", get(file_api::file_tree))
            .route("/api/files/content", get(file_api::file_content))
            .route("/api/commands", get(super::commands::handle_list_commands))
            .route("/api/commands/:name/execute", post(super::commands::handle_execute_command))
            .route("/api/themes", get(handle_list_themes))
            .route("/api/models", get(super::api::chat_api::handle_list_models_endpoint))
            .route("/api/model/switch", post(super::api::chat_api::handle_switch_model))
            .route("/api/logs", get(crate::server::api::log_api::handle_list_logs))
            .route("/api/logs/stream", get(crate::server::api::log_api::handle_log_stream))
            .layer(cors_layer(self.state.config.clone()))
            .with_state(self.state.clone());

        let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", self.port))
            .await
            .unwrap();

        println!("🚀 Server listening on http://0.0.0.0:{}", self.port);

        axum::serve(listener, app).await.unwrap();
    }
}

fn build_cors_layer(origins: &[String]) -> CorsLayer {
    let mut layer = CorsLayer::new();
    for origin in origins {
        let Ok(val) = origin.parse::<HeaderValue>() else { continue };
        layer = layer.allow_origin(val);
    }
    layer
        .allow_methods([axum::http::Method::GET, axum::http::Method::POST])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE])
}

/// CORS 中间件配置
fn cors_layer(config: Arc<RwLock<Config>>) -> CorsLayer {
    let cfg = config.read().unwrap();
    let Some(server_cfg) = &cfg.server else { return CorsLayer::permissive() };
    let Some(origins) = &server_cfg.allowed_origins else { return CorsLayer::permissive() };
    build_cors_layer(origins)
}

/// JSON-RPC 端点处理器
async fn handle_rpc_endpoint(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<JsonRpcRequest>,
) -> Json<JsonRpcResponse> {
    // 认证检查
    if let Some(resp) = check_auth(&headers, &state.config).await {
        return Json(resp);
    }

    let resp = handle_rpc(req, state.provider.clone(), state.config.clone()).await;
    Json(resp)
}

/// 认证检查
pub(crate) async fn check_auth(headers: &HeaderMap, config: &Arc<RwLock<Config>>) -> Option<JsonRpcResponse> {
    let cfg = config.read().ok()?;
    let server_cfg = cfg.server.as_ref()?;
    let expected_token = server_cfg.api_token.as_ref()?;

    let auth = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if !auth.starts_with("Bearer ") || auth.len() <= 7 || &auth[7..] != expected_token {
        return Some(JsonRpcResponse::error(
            -32000,
            "Unauthorized",
            Some(Value::Null),
        ));
    }

    None
}



/// 列出所有可用主题
async fn handle_list_themes(
    State(state): State<AppState>,
) -> Json<ApiResponse<Vec<crate::tui::theme::ThemePreset>>> {
    Json(ApiResponse::success(state.themes.clone()))
}


