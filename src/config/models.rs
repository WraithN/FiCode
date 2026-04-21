use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct Config {
    pub model: String,
    pub provider: HashMap<String, ProviderConfig>,
    pub mcp: Option<HashMap<String, McpServerConfig>>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct McpServerConfig {
    #[serde(rename = "type")]
    pub server_type: McpServerType,
    pub enabled: bool,
    pub command: Option<Vec<String>>,
    pub url: Option<String>,
    pub headers: Option<HashMap<String, String>>,
}

impl Default for McpServerConfig {
    fn default() -> Self {
        Self {
            server_type: McpServerType::Local,
            enabled: true,
            command: None,
            url: None,
            headers: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum McpServerType {
    Local,
    Remote,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct ProviderConfig {
    pub npm: String,
    pub name: String,
    pub options: ProviderOptions,
    pub models: HashMap<String, ModelConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct ProviderOptions {
    #[serde(rename = "apiKey")]
    pub api_key: String,
    #[serde(rename = "baseURL")]
    pub base_url: String,
    pub timeout: u64,
    #[serde(rename = "chunkTimeout")]
    pub chunk_timeout: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct ModelConfig {
    pub name: String,
    pub limit: ModelLimits,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct ModelLimits {
    pub context: u32,
    pub output: u32,
}
