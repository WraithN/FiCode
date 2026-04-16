// =============================================================================
// prompt 模块：系统提示词构建器
// =============================================================================
// 负责根据可用工具 schema 动态组装 System Prompt，让 Agent 明确自身能力边界。

const PROMPT_TEMPLATE: &str = r#"You are an autonomous coding assistant running in a terminal environment.

Your mission is to help the user with software engineering tasks by reasoning step-by-step, taking action when necessary, and reporting results clearly.

You have access to the following tools (described in JSON Schema):
{tools_schema}

Rules:
1. Analyze the user's request carefully before acting.
2. If a task requires file inspection, use `read` or `grep`.
3. If a task requires changing files, use `write` or `edit`.
4. If a task requires running commands (builds, tests, etc.), use `bash`.
5. When you need to fetch documentation from the web, use `web_fetch`.
6. Always prefer concrete actions over long explanations.
7. When you invoke a tool, wait for its result before proceeding to the next step.
8. If no tool is needed, reply directly to the user in a concise and helpful manner.
"#;

/// 系统提示词构建器。
pub struct PromptBuilder;

impl PromptBuilder {
    /// 创建一个新的提示词构建器。
    pub fn new() -> Self {
        Self
    }

    /// 根据工具 JSON Schema 构建系统提示词。
    ///
    /// # Arguments
    /// * `tools_schema` - 工具的 JSON Schema 描述
    pub fn build(&self, tools_schema: &serde_json::Value) -> String {
        let tools_str = serde_json::to_string_pretty(tools_schema).unwrap_or_default();
        PROMPT_TEMPLATE.replace("{tools_schema}", &tools_str)
    }
}

impl Default for PromptBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prompt_builder_includes_schema() {
        let builder = PromptBuilder::new();
        let schema = serde_json::json!([
            {
                "name": "bash",
                "description": "Run shell commands"
            }
        ]);
        let prompt = builder.build(&schema);
        assert!(prompt.contains("You are an autonomous coding assistant"));
        assert!(prompt.contains("\"name\": \"bash\""));
        assert!(prompt.contains("Run shell commands"));
        assert!(prompt.contains("Rules:"));
    }

    #[test]
    fn test_prompt_builder_empty_schema() {
        let builder = PromptBuilder::default();
        let prompt = builder.build(&serde_json::json!([]));
        assert!(prompt.contains("You are an autonomous coding assistant"));
        assert!(prompt.contains("[]"));
    }
}
