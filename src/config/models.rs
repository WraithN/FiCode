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
use std::collections::HashMap;

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct Config {
    pub model: String,
    pub provider: HashMap<String, ProviderConfig>,
    pub mcp: Option<HashMap<String, McpServerConfig>>,
    pub server: Option<ServerConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct ServerConfig {
    pub port: Option<u16>,
    pub api_token: Option<String>,
    pub allowed_origins: Option<Vec<String>>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: Some(4040),
            api_token: None,
            allowed_origins: None,
        }
    }
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
