use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::{Arc, RwLock};

use crate::config::Config;
use crate::provider::Provider;

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
    pub id: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
    pub id: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl JsonRpcResponse {
    pub fn success(result: Value, id: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: Some(result),
            error: None,
            id,
        }
    }

    pub fn error(code: i32, message: impl Into<String>, id: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
                data: None,
            }),
            id,
        }
    }
}

/// 处理 JSON-RPC 请求
pub async fn handle_rpc(
    req: JsonRpcRequest,
    provider: Arc<RwLock<Provider>>,
    config: Arc<RwLock<Config>>,
) -> JsonRpcResponse {
    if req.jsonrpc != "2.0" {
        return JsonRpcResponse::error(-32600, "Invalid Request", req.id);
    }

    match req.method.as_str() {
        "execute" => handle_execute(req.params, provider, config).await,
        "list_models" => handle_list_models(provider, config).await,
        "get_status" => handle_get_status(provider, config).await,
        _ => JsonRpcResponse::error(-32601, "Method not found", req.id),
    }
}

async fn handle_execute(
    params: Option<Value>,
    provider: Arc<RwLock<Provider>>,
    config: Arc<RwLock<Config>>,
) -> JsonRpcResponse {
    let id = params.as_ref().and_then(|p| p.get("id")).cloned();
    let command = match params.and_then(|p| p.get("command").and_then(|v| v.as_str().map(|s| s.to_string()))) {
        Some(cmd) => cmd,
        None => return JsonRpcResponse::error(-32602, "Missing 'command' parameter", id),
    };

    let slash_cmd = crate::commands::slash::parse(&command);
    if matches!(slash_cmd, crate::commands::slash::SlashCommand::Unknown(ref s) if s.is_empty()) {
        return JsonRpcResponse::error(-32602, "Not a valid command", id);
    }

    let handler = crate::commands::slash::SlashCommandHandler::new(provider, config);
    match handler.execute(slash_cmd).await {
        Ok(crate::commands::slash::SlashCommandResult::Handled) => {
            JsonRpcResponse::success(serde_json::json!({ "success": true, "message": "Executed" }), id)
        }
        Ok(crate::commands::slash::SlashCommandResult::Passthrough(_)) => {
            JsonRpcResponse::error(-32602, "Not a command", id)
        }
        Err(e) => JsonRpcResponse::error(-32603, format!("Execution failed: {}", e), id),
    }
}

async fn handle_list_models(
    provider: Arc<RwLock<Provider>>,
    config: Arc<RwLock<Config>>,
) -> JsonRpcResponse {
    let id = None;
    let cfg = match config.read() {
        Ok(c) => c,
        Err(_) => return JsonRpcResponse::error(-32603, "Config lock poisoned", id),
    };
    let provider_guard = match provider.read() {
        Ok(p) => p,
        Err(_) => return JsonRpcResponse::error(-32603, "Provider lock poisoned", id),
    };

    let models = provider_guard.list_models(&cfg);
    let model_list: Vec<Value> = models
        .into_iter()
        .map(|(key, name)| {
            serde_json::json!({
                "key": key,
                "name": name
            })
        })
        .collect();

    JsonRpcResponse::success(serde_json::json!({ "models": model_list }), id)
}

async fn handle_get_status(
    provider: Arc<RwLock<Provider>>,
    _config: Arc<RwLock<Config>>,
) -> JsonRpcResponse {
    let id = None;
    let current_model = match provider.read() {
        Ok(p) => p.model_name().unwrap_or("unknown").to_string(),
        Err(_) => "unknown".to_string(),
    };

    JsonRpcResponse::success(
        serde_json::json!({
            "status": "running",
            "version": env!("CARGO_PKG_VERSION"),
            "current_model": current_model,
        }),
        id,
    )
}
