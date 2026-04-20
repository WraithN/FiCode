use anyhow::{anyhow, Context, Result};
use std::fs;
use std::path::PathBuf;

use super::models::Config;

impl Config {
    /// 返回配置目录路径：~/.config/shun-code/
    pub fn config_dir() -> PathBuf {
        directories::ProjectDirs::from("", "", "shun-code")
            .map(|d| d.config_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from(".config/shun-code"))
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
