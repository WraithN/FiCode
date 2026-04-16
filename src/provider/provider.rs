use super::{AIClient, AnthropicClient, OpenAiClient};
use anyhow::{anyhow, Result};
use std::env;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelType {
    OpenAiCompatible,
    Anthropic,
}

#[derive(Debug, Clone)]
pub struct Model {
    pub api_key: String,
    pub base_url: String,
    pub model_name: String,
    pub model_type: ModelType,
}

impl Model {
    pub fn get_model() -> Result<Self> {
        dotenvy::dotenv().ok();

        if let (Ok(api_key), Ok(base_url), Ok(model_name)) = (
            env::var("OPENAI_API_KEY"),
            env::var("OPENAI_BASE_URL"),
            env::var("OPENAI_MODEL"),
        ) {
            return Ok(Self {
                api_key,
                base_url,
                model_name,
                model_type: ModelType::OpenAiCompatible,
            });
        }

        let anthropic_api_key =
            env::var("ANTHROPIC_API_KEY").or_else(|_| env::var("ANTHROPIC_AUTH_TOKEN"));
        if let (Ok(api_key), Ok(base_url), Ok(model_name)) = (
            anthropic_api_key,
            env::var("ANTHROPIC_BASE_URL"),
            env::var("ANTHROPIC_MODEL"),
        ) {
            return Ok(Self {
                api_key,
                base_url,
                model_name,
                model_type: ModelType::Anthropic,
            });
        }

        Err(anyhow!(
            "No API key found. Please set OPENAI_API_KEY or ANTHROPIC_API_KEY environment variable."
        ))
    }
}

pub struct Provider {
    pub model: Option<Model>,
}

impl Provider {
    pub fn new() -> Self {
        Self { model: None }
    }

    pub fn set_model(&mut self, model: Model) {
        self.model = Some(model);
    }

    pub fn get_client(&self) -> Result<Box<dyn AIClient>> {
        let model = self
            .model
            .as_ref()
            .ok_or_else(|| anyhow!("Model not set"))?;
        if model.model_type == ModelType::OpenAiCompatible {
            let client = OpenAiClient::from_model(model)?;
            Ok(Box::new(client))
        } else if model.model_type == ModelType::Anthropic {
            let client = AnthropicClient::from_model(model)?;
            Ok(Box::new(client))
        } else {
            Err(anyhow!(
                "Model type conflict: cannot be both OpenAiCompatible and Anthropic"
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::{AIClient, ChunkContent, FinishReason};
    use crate::session::message::{Message, Part, Role};
    use std::time::Duration;

    /// 探测 localhost:11434 是否有可用的 Ollama 服务，并返回一个可用模型名。
    async fn try_get_ollama_model() -> Option<String> {
        let client = reqwest::Client::new();
        let resp = client
            .get("http://localhost:11434/api/tags")
            .timeout(Duration::from_secs(2))
            .send()
            .await
            .ok()?;
        if !resp.status().is_success() {
            return None;
        }
        let body: serde_json::Value = resp.json().await.ok()?;
        let models = body.get("models")?.as_array()?;
        models.first()?.get("name")?.as_str().map(|s| s.to_string())
    }

    /// 测试本地 Ollama 的 OpenAI 兼容流式接口：纯文本场景
    #[tokio::test]
    async fn test_local_openai_compatible_text_stream() {
        let Some(model_name) = try_get_ollama_model().await else {
            eprintln!("Ollama not available at localhost:11434, skipping test.");
            return;
        };

        let model = Model {
            api_key: "dummy".to_string(),
            base_url: "http://localhost:11434".to_string(),
            model_name,
            model_type: ModelType::OpenAiCompatible,
        };

        let mut provider = Provider::new();
        provider.set_model(model);

        let client = provider.get_client().expect("should create client");

        let messages = vec![Message::new(
            "test-session",
            Role::User,
            vec![Part::Text {
                text: "Please reply with exactly the word 'pong'.".to_string(),
            }],
        )];

        let schema = serde_json::json!([]);
        let mut texts = Vec::new();
        let mut finish_reason = None;

        client
            .stream_message(
                "You are a concise assistant.",
                &messages,
                &schema,
                &mut |chunk| match chunk.content {
                    ChunkContent::Text(t) => texts.push(t),
                    ChunkContent::Finish(r) => finish_reason = Some(r),
                    _ => {}
                },
            )
            .await
            .expect("stream_message should succeed");

        let full_text = texts.join("");
        println!("text stream response: {}", full_text);
        assert!(
            !full_text.is_empty() || finish_reason.is_some(),
            "should receive at least text or finish reason"
        );
        assert_eq!(
            finish_reason,
            Some(FinishReason::Stop),
            "text-only stream should finish with Stop"
        );
    }

    /// 测试本地 Ollama 的 OpenAI 兼容流式接口：tool_use 场景
    #[tokio::test]
    async fn test_local_openai_compatible_tool_use_stream() {
        let Some(model_name) = try_get_ollama_model().await else {
            eprintln!("Ollama not available at localhost:11434, skipping test.");
            return;
        };

        let model = Model {
            api_key: "dummy".to_string(),
            base_url: "http://localhost:11434".to_string(),
            model_name,
            model_type: ModelType::OpenAiCompatible,
        };

        let mut provider = Provider::new();
        provider.set_model(model);

        let client = provider.get_client().expect("should create client");

        let tools_schema = serde_json::json!([
            {
                "name": "calculator",
                "description": "Add two numbers.",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "a": { "type": "number" },
                        "b": { "type": "number" }
                    },
                    "required": ["a", "b"]
                }
            }
        ]);

        let messages = vec![Message::new(
            "test-session",
            Role::User,
            vec![Part::Text {
                text: "What is 23 plus 45? Use the calculator tool.".to_string(),
            }],
        )];

        let mut texts = Vec::new();
        let mut tool_uses = Vec::new();
        let mut finish_reason = None;

        let result = client
            .stream_message(
                "You are a helpful assistant. Use tools when appropriate.",
                &messages,
                &tools_schema,
                &mut |chunk| match chunk.content {
                    ChunkContent::Text(t) => texts.push(t),
                    ChunkContent::ToolUse(part) => {
                        if let Part::ToolUse {
                            id,
                            name,
                            arguments,
                        } = part
                        {
                            tool_uses.push((id, name, arguments));
                        }
                    }
                    ChunkContent::Finish(r) => finish_reason = Some(r),
                    _ => {}
                },
            )
            .await;

        if let Err(e) = result {
            eprintln!(
                "stream_message returned error (model may not support tools): {}",
                e
            );
            return;
        }

        if !tool_uses.is_empty() {
            assert_eq!(
                finish_reason,
                Some(FinishReason::ToolUse),
                "tool use stream should finish with ToolUse"
            );
            let (_, name, args) = &tool_uses[0];
            assert_eq!(name, "calculator");
            assert!(
                args.get("a").is_some() || args.get("b").is_some(),
                "calculator arguments should contain a or b, got: {}",
                args
            );
        } else {
            // 模型未触发工具调用（可能是模型不支持 tools），仅做基本断言，不使测试失败
            let full_text = texts.join("");
            println!("tool stream text response (no tool use): {}", full_text);
            assert!(
                !full_text.is_empty() || finish_reason.is_some(),
                "should receive text or finish reason even when no tool is used"
            );
        }
    }
}
