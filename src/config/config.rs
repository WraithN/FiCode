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

use anyhow::{anyhow, Context, Result};
use notify::Watcher;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use crate::log_info;
use super::models::Config;

impl Config {
    /// 返回配置目录路径：~/.config/fi-code/
    pub fn config_dir() -> PathBuf {
        directories::ProjectDirs::from("", "", "fi-code")
            .map(|d| d.config_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from(".config/fi-code"))
    }

    /// 加载配置文件，支持 .jsonc 和 .json
    pub fn load() -> Result<Self> {
        let config_dir = Self::config_dir();
        let paths = [
            config_dir.join("config.jsonc"),
            config_dir.join("config.json"),
        ];

        for path in &paths {
            if path.exists() {
                let content = fs::read_to_string(path)
                    .with_context(|| format!("无法读取配置文件: {:?}", path))?;
                let is_jsonc = path.extension().map(|e| e == "jsonc").unwrap_or(false);
                return Self::parse(&content, is_jsonc);
            }
        }

        Ok(Config::default())
    }

    pub fn parse(content: &str, is_jsonc: bool) -> Result<Self> {
        let mut config: Config = if is_jsonc {
            jsonc_parser::parse_to_serde_value(content, &Default::default())
                .map_err(|e| anyhow!("JSONC 解析失败: {}", e))?
        } else {
            serde_json::from_str(content).with_context(|| "配置文件格式错误")?
        };
        config.resolve_env_placeholders()?;
        Ok(config)
    }

    pub fn resolve_env_placeholders(&mut self) -> Result<()> {
        for (_, provider) in &mut self.provider {
            if provider.options.api_key.starts_with("{env:") {
                let var_name = extract_env_var(&provider.options.api_key)?;
                provider.options.api_key = std::env::var(&var_name)
                    .with_context(|| format!("环境变量 {} 未设置", var_name))?;
            }
        }
        Ok(())
    }
}

fn extract_env_var(placeholder: &str) -> Result<String> {
    let start = placeholder
        .find("{env:")
        .ok_or_else(|| anyhow!("无效的环境变量占位符"))?
        + 5;
    let end = placeholder
        .find('}')
        .ok_or_else(|| anyhow!("占位符缺少闭合括号"))?;
    Ok(placeholder[start..end].to_string())
}

fn try_reload_config(
    res: Result<notify::Event, notify::Error>,
    last_reload: &Mutex<Instant>,
    config: &Arc<RwLock<Config>>,
) {
    let Ok(event) = res else { return };
    if !event.kind.is_modify() {
        return;
    }

    let mut last = last_reload.lock().unwrap();
    if last.elapsed() < Duration::from_millis(500) {
        return;
    }
    *last = Instant::now();
    drop(last);

    let Ok(new_config) = Config::load() else {
        log_info!("Warning: 配置热重载失败");
        return;
    };

    let Ok(mut cfg) = config.write() else {
        log_info!("Warning: 配置锁中毒，无法更新");
        return;
    };

    *cfg = new_config;
    log_info!("配置已热重载");
}

pub fn spawn_watcher(config: Arc<RwLock<Config>>) -> Result<impl notify::Watcher> {
    let config_dir = Config::config_dir();
    let last_reload = Arc::new(Mutex::new(Instant::now()));
    let last_reload_clone = Arc::clone(&last_reload);
    let config_clone = Arc::clone(&config);

    let mut watcher = notify::recommended_watcher(move |res| {
        try_reload_config(res, &last_reload_clone, &config_clone);
    })?;

    watcher.watch(&config_dir, notify::RecursiveMode::NonRecursive)?;
    Ok(watcher)
}
