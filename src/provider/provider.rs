use anyhow::{anyhow, Result};
use std::env;
use super::{AnthropicClient, OpenAiClient, AIClient};

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

        let anthropic_api_key = env::var("ANTHROPIC_API_KEY")
            .or_else(|_| env::var("ANTHROPIC_AUTH_TOKEN"));
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
        Self {
            model: None,
        }
    }

    pub fn set_model(&mut self, model: Model) {
        self.model = Some(model);
    }

    pub fn get_client(&self) -> Result<Box<dyn AIClient>> {
        let model = self.model.as_ref().ok_or_else(|| anyhow!("Model not set"))?;
        if model.model_type == ModelType::OpenAiCompatible {
            let client = OpenAiClient::from_model(model)?;
            Ok(Box::new(client))
        } else if model.model_type == ModelType::Anthropic {
            let client = AnthropicClient::from_model(model)?;
            Ok(Box::new(client))
        } else {
            Err(anyhow!("Model type conflict: cannot be both OpenAiCompatible and Anthropic"))
        }
    }
}
