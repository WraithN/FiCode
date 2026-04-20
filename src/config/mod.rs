pub mod config;
pub mod models;

pub use models::{Config, ModelConfig, ModelLimits, ProviderConfig, ProviderOptions};

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert!(config.model.is_empty());
        assert!(config.provider.is_empty());
    }

    #[test]
    fn test_parse_json_config() {
        let json = r#"{
            "model": "my-model",
            "provider": {
                "openai": {
                    "npm": "@ai-sdk/openai-compatible",
                    "name": "OpenAI",
                    "options": {
                        "apiKey": "sk-test",
                        "baseURL": "https://api.openai.com/v1",
                        "timeout": 300000,
                        "chunkTimeout": 10000
                    },
                    "models": {
                        "my-model": {
                            "name": "My Model",
                            "limit": { "context": 200000, "output": 65536 }
                        }
                    }
                }
            }
        }"#;

        let config = Config::parse(json, false).unwrap();
        assert_eq!(config.model, "my-model");
        assert!(config.provider.contains_key("openai"));
    }

    #[test]
    fn test_parse_jsonc_with_comments() {
        let jsonc = r#"{
            // 默认模型
            "model": "my-model",
            "provider": {
                "openai": {
                    "npm": "@ai-sdk/openai-compatible",
                    "name": "OpenAI",
                    "options": {
                        "apiKey": "sk-test",
                        "baseURL": "https://api.openai.com/v1",
                        "timeout": 300000,
                        "chunkTimeout": 10000
                    },
                    "models": {
                        "my-model": {
                            "name": "My Model",
                            "limit": { "context": 200000, "output": 65536 }
                        }
                    }
                }
            }
        }"#;

        let config = Config::parse(jsonc, true).unwrap();
        assert_eq!(config.model, "my-model");
    }

    #[test]
    fn test_env_placeholder_resolution() {
        std::env::set_var("TEST_API_KEY", "resolved-key");

        let json = r#"{
            "model": "test-model",
            "provider": {
                "test": {
                    "npm": "test",
                    "name": "Test",
                    "options": {
                        "apiKey": "{env:TEST_API_KEY}",
                        "baseURL": "https://test.com",
                        "timeout": 1000,
                        "chunkTimeout": 1000
                    },
                    "models": {
                        "test-model": {
                            "name": "Test Model",
                            "limit": { "context": 1000, "output": 1000 }
                        }
                    }
                }
            }
        }"#;

        let mut config = Config::parse(json, false).unwrap();
        config.resolve_env_placeholders().unwrap();

        let provider = config.provider.get("test").unwrap();
        assert_eq!(provider.options.api_key, "resolved-key");
    }
}
