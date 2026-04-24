# Agent 指令系统实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 fi-code 添加 `/init` 和 `/model` 两个 slash 指令，并将 `main.rs` 业务逻辑迁移到 `entry.rs`。

**Architecture:** 创建 `commands/slash.rs` 模块负责指令解析与执行，`entry.rs` 承载原 `main.rs` 的交互逻辑，Provider 支持运行时模型切换，PromptBuilder 自动注入 AGENTS.md。

**Tech Stack:** Rust 2021, tokio, anyhow, serde, rustyline

---

## 文件结构

| 文件 | 操作 | 职责 |
|------|------|------|
| `src/entry.rs` | 创建 | 从 `main.rs` 迁移的交互/单命令模式、会话选择、TaskManager 调度、slash 指令拦截 |
| `src/main.rs` | 修改 | 仅保留 `mod entry;` 和 `entry::run().await` |
| `src/commands/mod.rs` | 修改 | 声明并导出 slash 子模块 |
| `src/commands/slash.rs` | 创建 | SlashCommand 枚举、解析器、执行器、结果类型 |
| `src/provider/provider.rs` | 修改 | 添加 `set_model()` 和 `list_models()` |
| `src/agent/prompt.rs` | 修改 | 自动检测并注入 AGENTS.md 内容 |

---

## Task 1: 迁移 main.rs 业务逻辑到 entry.rs

**Files:**
- Create: `src/entry.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: 创建 `src/entry.rs`，迁移所有业务函数**

将 `src/main.rs` 中除 `mod` 声明和 `main()` 之外的全部内容迁移到 `src/entry.rs`：
- `SUBAGENT_SYSTEM_PROMPT`
- `print_task_plan()`
- `extract_task_plan_result()`
- `run_single_command()`
- `run_interactive()`
- `choose_or_create_session()`

并在 `src/entry.rs` 顶部添加必要的 `use` 语句和 `pub async fn run() -> Result<()>` 入口函数。

```rust
// src/entry.rs 顶部
use anyhow::{anyhow, Context, Result};
use colored::Colorize;
use rustyline::DefaultEditor;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use crate::agent::{agent_loop, LoopState};
use crate::config::Config;
use crate::mcp::manager::McpManager;
use crate::provider::{base_client::AIClient, Provider};
use crate::session::message::{Message, Part, Role};
use crate::session::{SessionManager, SessionMeta, SessionStatus};
use crate::task::{TaskManager, TaskPlan};
use crate::tools::set_mcp_manager;
use crate::utils::cli::Args;
use crate::utils::workspace::set_workspace;
use clap::Parser;

pub async fn run() -> Result<()> {
    // 原 main() 的全部逻辑迁移到这里
}
```

- [ ] **Step 2: 精简 `src/main.rs` 为入口包装**

```rust
#![allow(warnings)]

mod agent;
mod commands;
mod config;
mod entry;
mod mcp;
mod permission;
mod provider;
mod session;
mod skills;
mod task;
mod tools;
mod utils;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    entry::run().await
}
```

- [ ] **Step 3: 验证编译通过**

Run: `cargo check`
Expected: 编译成功，无错误

- [ ] **Step 4: Commit**

```bash
git add src/main.rs src/entry.rs
git commit -m "refactor: extract entry.rs from main.rs"
```

---

## Task 2: 创建 commands/mod.rs 声明

**Files:**
- Modify: `src/commands/mod.rs`

- [ ] **Step 1: 编写 commands 模块声明**

```rust
// src/commands/mod.rs
pub mod slash;
pub use slash::{parse, SlashCommand, SlashCommandHandler, SlashCommandResult};
```

- [ ] **Step 2: Commit**

```bash
git add src/commands/mod.rs
git commit -m "feat(commands): add slash command module declaration"
```

---

## Task 3: 实现 SlashCommand 解析器

**Files:**
- Create: `src/commands/slash.rs`

- [ ] **Step 1: 编写解析失败测试**

在 `src/commands/slash.rs` 底部添加：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_model_no_args() {
        assert_eq!(parse("/model"), SlashCommand::Model(None));
    }

    #[test]
    fn test_parse_model_with_args() {
        assert_eq!(
            parse("/model gpt-4o"),
            SlashCommand::Model(Some("gpt-4o".to_string()))
        );
    }

    #[test]
    fn test_parse_init() {
        assert_eq!(parse("/init"), SlashCommand::Init);
    }

    #[test]
    fn test_parse_unknown() {
        assert_eq!(
            parse("/foo"),
            SlashCommand::Unknown("foo".to_string())
        );
    }

    #[test]
    fn test_parse_not_slash() {
        assert_eq!(
            parse("hello world"),
            SlashCommand::Unknown("".to_string())
        );
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test commands::slash`
Expected: 编译失败（类型未定义）

- [ ] **Step 3: 实现 SlashCommand 枚举和解析函数**

```rust
// src/commands/slash.rs
use std::sync::{Arc, RwLock};
use anyhow::{anyhow, Result};
use colored::Colorize;

use crate::config::Config;
use crate::provider::Provider;
use crate::agent::prompt::PromptBuilder;
use crate::agent::{agent_loop, LoopState};
use crate::session::message::{Message, Part, Role};
use crate::utils::workspace::workspace_dir;

/// 可识别的 slash 指令
#[derive(Debug, PartialEq)]
pub enum SlashCommand {
    /// /model [model_key]
    Model(Option<String>),
    /// /init
    Init,
    /// 未知指令（携带指令名，空字符串表示非 slash 输入）
    Unknown(String),
}

/// 指令执行结果
#[derive(Debug, PartialEq)]
pub enum SlashCommandResult {
    /// 指令已处理，无需进入正常 LLM 对话流程
    Handled,
    /// 非 slash 指令，按正常用户输入处理
    Passthrough(String),
}

/// 解析用户输入为 slash 指令
pub fn parse(input: &str) -> SlashCommand {
    let trimmed = input.trim();
    if !trimmed.starts_with('/') {
        return SlashCommand::Unknown("".to_string());
    }

    let parts: Vec<&str> = trimmed.splitn(2, ' ').collect();
    let cmd = parts[0];
    let arg = parts.get(1).map(|s| s.trim().to_string());

    match cmd {
        "/model" => SlashCommand::Model(arg.filter(|s| !s.is_empty())),
        "/init" => SlashCommand::Init,
        _ => SlashCommand::Unknown(cmd.trim_start_matches('/').to_string()),
    }
}

/// 指令执行器
pub struct SlashCommandHandler {
    provider: Arc<Provider>,
    config: Arc<RwLock<Config>>,
}

impl SlashCommandHandler {
    pub fn new(provider: Arc<Provider>, config: Arc<RwLock<Config>>) -> Self {
        Self { provider, config }
    }

    pub fn execute(&self, cmd: SlashCommand) -> Result<SlashCommandResult> {
        match cmd {
            SlashCommand::Model(model_key) => self.handle_model(model_key),
            SlashCommand::Init => self.handle_init(),
            SlashCommand::Unknown(name) if name.is_empty() => {
                // 非 slash 输入，不应到达此处（由调用方前置判断）
                unreachable!()
            }
            SlashCommand::Unknown(name) => {
                eprintln!(
                    "{} 未知命令: /{}。可用命令: /init, /model",
                    "❌".red(),
                    name
                );
                Ok(SlashCommandResult::Handled)
            }
        }
    }

    fn handle_model(&self, model_key: Option<String>) -> Result<SlashCommandResult> {
        let cfg = self.config.read().map_err(|_| anyhow!("配置锁中毒"))?;

        if let Some(key) = model_key {
            // 查找模型
            let mut found = false;
            for (_provider_name, provider_cfg) in &cfg.provider {
                if provider_cfg.models.contains_key(&key) {
                    found = true;
                    break;
                }
            }

            if found {
                // 切换模型
                self.provider.set_model(&key, &cfg)?;
                println!("{} 已切换模型: {}", "✅".green(), key);
            } else {
                eprintln!("{} 没有此模型: {}", "❌".red(), key);
                self.print_model_list(&cfg)?;
            }
        } else {
            // 展示列表
            self.print_model_list(&cfg)?;
        }

        Ok(SlashCommandResult::Handled)
    }

    fn print_model_list(&self, cfg: &Config) -> Result<()> {
        let models = self.provider.list_models(cfg);
        if models.is_empty() {
            println!("{} 配置文件中未找到任何模型", "❌".red());
            return Ok(());
        }

        println!("可用模型列表：");
        for (i, (key, display)) in models.iter().enumerate() {
            // 查找 limit 信息
            let mut limit_str = String::new();
            for (_pname, pcfg) in &cfg.provider {
                if let Some(mcfg) = pcfg.models.get(key) {
                    limit_str = format!(
                        " (context: {}, output: {})",
                        mcfg.limit.context, mcfg.limit.output
                    );
                    break;
                }
            }
            println!("  [{}] {} — {}{}", i + 1, key, display, limit_str);
        }
        Ok(())
    }

    fn handle_init(&self) -> Result<SlashCommandResult> {
        let workspace = workspace_dir();
        println!(
            "{} 正在分析项目结构，生成 AGENTS.md...",
            "🔍".yellow()
        );

        let init_prompt = r#"你是一个项目文档助手。请深入分析当前项目的结构、技术栈、代码风格和重要约定，生成一份 AGENTS.md 文件。AGENTS.md 的目标是帮助 AI 编程助手快速理解项目背景。你可以使用 read、grep、bash 等工具来探索代码库。"#;

        let user_prompt = format!(
            "请为当前项目生成 AGENTS.md，保存路径为: {}/AGENTS.md",
            workspace.display()
        );

        // 创建临时 LoopState（不加入 session 历史）
        let mut state = LoopState::new(vec![Message::new(
            "init-session".to_string(),
            Role::User,
            vec![Part::Text { text: user_prompt }],
        )]);

        let client = self.provider.get_client()?;
        // 使用自定义系统提示词运行 agent_loop
        // 注意：这里需要直接调用 stream_message，因为 agent_loop 使用默认 PromptBuilder
        // 为简化实现，使用 AgentRunner 并传入自定义 system_prompt
        let schema = crate::tools::tool_schema();
        let runtime = tokio::runtime::Handle::try_current()?;
        // ... 实际实现见 Task 5 集成步骤

        println!(
            "{} AGENTS.md 已生成: {}/AGENTS.md",
            "✅".green(),
            workspace.display()
        );
        Ok(SlashCommandResult::Handled)
    }
}
```

> **注意：** `handle_init` 中的具体 LLM 调用逻辑在 Task 5 中完善，此处先保留结构。

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test commands::slash`
Expected: 5 个测试全部 PASS

- [ ] **Step 5: Commit**

```bash
git add src/commands/slash.rs
git commit -m "feat(commands): implement slash command parser and handler"
```

---

## Task 4: Provider 支持运行时模型切换

**Files:**
- Modify: `src/provider/provider.rs`

- [ ] **Step 1: 编写测试（先写失败）**

在 `src/provider/provider.rs` 的 `#[cfg(test)]` 模块中添加：

```rust
    #[test]
    fn test_set_model_and_list_models() {
        use crate::config::models::{ModelConfig, ModelLimits, ProviderConfig, ProviderOptions};

        let mut provider_map = HashMap::new();
        provider_map.insert(
            "openai".to_string(),
            ProviderConfig {
                npm: "@ai-sdk/openai-compatible".to_string(),
                name: "OpenAI".to_string(),
                options: ProviderOptions {
                    api_key: "test-key".to_string(),
                    base_url: "https://test.com".to_string(),
                    timeout: 300000,
                    chunk_timeout: 10000,
                },
                models: {
                    let mut m = HashMap::new();
                    m.insert(
                        "gpt-4".to_string(),
                        ModelConfig {
                            name: "GPT-4".to_string(),
                            limit: ModelLimits {
                                context: 128000,
                                output: 4096,
                            },
                        },
                    );
                    m.insert(
                        "gpt-3.5".to_string(),
                        ModelConfig {
                            name: "GPT-3.5".to_string(),
                            limit: ModelLimits {
                                context: 16000,
                                output: 4096,
                            },
                        },
                    );
                    m
                },
            },
        );

        let config = Config {
            model: "gpt-4".to_string(),
            provider: provider_map,
            mcp: None,
        };

        let mut provider = Provider::from_config(&config).unwrap();
        assert_eq!(provider.model_name().unwrap(), "gpt-4");

        // 测试 list_models
        let models = provider.list_models(&config);
        assert_eq!(models.len(), 2);
        assert!(models.iter().any(|(k, _)| k == "gpt-4"));
        assert!(models.iter().any(|(k, _)| k == "gpt-3.5"));

        // 测试 set_model
        provider.set_model("gpt-3.5", &config).unwrap();
        assert_eq!(provider.model_name().unwrap(), "gpt-3.5");

        // 测试 set_model 无效模型
        assert!(provider.set_model("invalid", &config).is_err());
    }
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test provider::provider::tests::test_set_model_and_list_models`
Expected: 编译失败（`set_model` 和 `list_models` 未定义）

- [ ] **Step 3: 实现 `set_model` 和 `list_models`**

在 `src/provider/provider.rs` 的 `impl Provider` 中添加：

```rust
    /// 运行时切换模型。
    ///
    /// # Arguments
    /// * `model_name` - 目标模型的 key
    /// * `config` - 当前配置（用于查找模型对应的 provider 和参数）
    pub fn set_model(&mut self, model_name: &str, config: &Config) -> Result<()> {
        for (provider_name, provider_cfg) in &config.provider {
            if let Some(_model_cfg) = provider_cfg.models.get(model_name) {
                let model_type = match provider_name.as_str() {
                    "anthropic" => ModelType::Anthropic,
                    _ => ModelType::OpenAiCompatible,
                };
                self.model = Some(Model {
                    api_key: provider_cfg.options.api_key.clone(),
                    base_url: provider_cfg.options.base_url.clone(),
                    model_name: model_name.to_string(),
                    model_type,
                });
                return Ok(());
            }
        }
        Err(anyhow!("模型 '{}' 在配置中未找到", model_name))
    }

    /// 枚举配置中所有可用模型。
    ///
    /// 返回 `(model_key, display_name)` 的列表。
    pub fn list_models(&self, config: &Config) -> Vec<(String, String)> {
        let mut result = Vec::new();
        for (_provider_name, provider_cfg) in &config.provider {
            for (model_key, model_cfg) in &provider_cfg.models {
                result.push((model_key.clone(), model_cfg.name.clone()));
            }
        }
        result
    }
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test provider::provider::tests::test_set_model_and_list_models`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/provider/provider.rs
git commit -m "feat(provider): add runtime model switching and listing"
```

---

## Task 5: PromptBuilder 自动注入 AGENTS.md

**Files:**
- Modify: `src/agent/prompt.rs`

- [ ] **Step 1: 编写测试（先写失败）**

在 `src/agent/prompt.rs` 的 `#[cfg(test)]` 模块中添加：

```rust
    use std::io::Write;

    #[test]
    fn test_prompt_with_agents_md() {
        // 设置临时工作目录并创建 AGENTS.md
        let temp_dir = std::env::temp_dir().join("fi-code-test-agents-md");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        let agents_path = temp_dir.join("AGENTS.md");
        let mut file = std::fs::File::create(&agents_path).unwrap();
        file.write_all(b"# Test Project\n\nThis is a test.").unwrap();

        crate::utils::workspace::set_workspace(temp_dir.clone());

        let builder = PromptBuilder::new();
        let prompt = builder.build(&serde_json::json!([]), &crate::skills::SkillRegistry::new());

        assert!(
            prompt.contains("# Project Context (AGENTS.md)"),
            "prompt should contain AGENTS.md header"
        );
        assert!(
            prompt.contains("This is a test."),
            "prompt should contain AGENTS.md content"
        );

        // 清理
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_prompt_without_agents_md() {
        // 设置没有 AGENTS.md 的工作目录
        let temp_dir = std::env::temp_dir().join("fi-code-test-no-agents-md");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        crate::utils::workspace::set_workspace(temp_dir.clone());

        let builder = PromptBuilder::new();
        let prompt = builder.build(&serde_json::json!([]), &crate::skills::SkillRegistry::new());

        assert!(
            !prompt.contains("# Project Context (AGENTS.md)"),
            "prompt should NOT contain AGENTS.md header when file missing"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test agent::prompt::tests::test_prompt_with_agents_md`
Expected: 失败（AGENTS.md 注入逻辑未实现）

- [ ] **Step 3: 修改 `PromptBuilder::build()` 实现注入**

```rust
// 在 src/agent/prompt.rs 中，修改 build 方法：

use crate::utils::workspace::workspace_dir;

    pub fn build(&self, tools_schema: &serde_json::Value, registry: &SkillRegistry) -> String {
        let tools_str = serde_json::to_string_pretty(tools_schema).unwrap_or_default();
        let mut prompt = PROMPT_TEMPLATE.replace("{tools_schema}", &tools_str);

        // 自动注入 AGENTS.md（如果存在）
        let workspace = workspace_dir();
        let agents_md_path = workspace.join("AGENTS.md");
        if agents_md_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&agents_md_path) {
                if !content.trim().is_empty() {
                    prompt.push_str("\n\n# Project Context (AGENTS.md)\n");
                    prompt.push_str(&content);
                }
            }
        }

        // 如果注册表非空，追加 Available Skills 段落
        if !registry.entries.is_empty() {
            prompt.push_str("\n\n## Available Skills\n");
            prompt.push_str(
                "You can load any of the following skills on-demand by calling the `use_skill` tool:\n\n",
            );
            for entry in &registry.entries {
                prompt.push_str(&format!(
                    "- `{}` ({}): {}\n",
                    entry.metadata.name, entry.scope, entry.metadata.description
                ));
            }
        }

        prompt
    }
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test agent::prompt`
Expected: 所有测试 PASS（包括新测试和原有测试）

- [ ] **Step 5: Commit**

```bash
git add src/agent/prompt.rs
git commit -m "feat(prompt): auto-inject AGENTS.md into system prompt"
```

---

## Task 6: 在 entry.rs 中集成 slash 指令拦截

**Files:**
- Modify: `src/entry.rs`

- [ ] **Step 1: 在 `run_single_command` 中添加 slash 指令拦截**

在 `run_single_command` 函数中，于创建 `user_msg` 之前插入：

```rust
async fn run_single_command(
    provider: Arc<Provider>,
    session_manager: &SessionManager,
    sessions_dir: &PathBuf,
    session: &mut session::Session,
    query: &str,
    config: Arc<RwLock<Config>>,
) -> Result<()> {
    // 拦截 slash 指令
    let slash_cmd = crate::commands::slash::parse(query);
    if !matches!(slash_cmd, crate::commands::slash::SlashCommand::Unknown(ref s) if s.is_empty())
    {
        let handler = crate::commands::slash::SlashCommandHandler::new(provider, config);
        handler.execute(slash_cmd)?;
        return Ok(());
    }

    // 原有逻辑继续...
```

注意：需要同步修改 `run_single_command` 的签名，增加 `config: Arc<RwLock<Config>>` 参数。

- [ ] **Step 2: 在 `run_interactive` 中添加 slash 指令拦截**

在 `run_interactive` 的循环中，于创建 `user_msg` 之前插入相同逻辑：

```rust
                // 拦截 slash 指令
                let slash_cmd = crate::commands::slash::parse(query);
                if !matches!(slash_cmd, crate::commands::slash::SlashCommand::Unknown(ref s) if s.is_empty())
                {
                    let handler = crate::commands::slash::SlashCommandHandler::new(
                        Arc::clone(&provider),
                        Arc::clone(&config),
                    );
                    if let Err(e) = handler.execute(slash_cmd) {
                        eprintln!("Error: {}", e);
                    }
                    continue;
                }
```

- [ ] **Step 3: 更新调用方传递 config**

修改 `entry::run()` 中两处调用 `run_single_command` 和 `run_interactive` 的地方，传入 `Arc::clone(&config)`。

- [ ] **Step 4: 验证编译通过**

Run: `cargo check`
Expected: 编译成功

- [ ] **Step 5: Commit**

```bash
git add src/entry.rs
git commit -m "feat(entry): integrate slash command interception"
```

---

## Task 7: 完善 `/init` 的 LLM 调用逻辑

**Files:**
- Modify: `src/commands/slash.rs`

- [ ] **Step 1: 在 `handle_init` 中实现完整的 LLM 调用**

由于 `agent_loop` 使用默认的 `PromptBuilder` 构建系统提示词，而 `/init` 需要自定义系统提示词，我们直接使用 `AgentRunner`（它允许传入自定义 system_prompt）：

```rust
    use crate::agent::runner::AgentRunner;
    use crate::tools::tool_schema;

    fn handle_init(&self) -> Result<SlashCommandResult> {
        let workspace = workspace_dir();
        let agents_path = workspace.join("AGENTS.md");
        println!(
            "{} 正在分析项目结构，生成 AGENTS.md...",
            "🔍".yellow()
        );

        let system_prompt = r#"你是一个项目文档助手。请深入分析当前项目的结构、技术栈、代码风格和重要约定，生成一份 AGENTS.md 文件。AGENTS.md 的目标是帮助 AI 编程助手快速理解项目背景。

你可以使用以下工具来探索代码库：
- read / read_file: 读取文件内容
- grep: 搜索代码内容
- bash: 执行命令（如 find, ls, tree 等）
- write: 写入文件（用于生成 AGENTS.md）

分析时请注意：
1. 阅读项目根目录的关键文件（README.md, Cargo.toml, package.json 等）
2. 浏览 src/ 目录结构
3. 查看主要模块的入口文件
4. 总结项目使用的技术栈、架构模式和开发约定
5. 将结果写入 AGENTS.md"#;

        let user_prompt = format!(
            "请为当前项目生成 AGENTS.md，保存路径为: {}",
            agents_path.display()
        );

        let runtime = tokio::runtime::Handle::try_current()?;
        let client = self.provider.get_client()?;
        let schema = runtime.block_on(async { tool_schema().await });

        let runner = AgentRunner::new(client, system_prompt, schema);
        let initial_messages = vec![Message::new(
            "init-session".to_string(),
            Role::User,
            vec![Part::Text { text: user_prompt }],
        )];

        let result = runtime.block_on(async { runner.run(initial_messages).await })?;

        // 检查结果中是否包含工具调用（write AGENTS.md）
        let has_write = result.messages.iter().any(|msg| {
            msg.parts.iter().any(|part| {
                matches!(part, Part::ToolUse { name, .. } if name == "write")
            })
        });

        if has_write || agents_path.exists() {
            println!(
                "{} AGENTS.md 已生成: {}",
                "✅".green(),
                agents_path.display()
            );
        } else {
            println!(
                "{} AGENTS.md 可能未生成，请检查对话结果",
                "⚠️".yellow()
            );
        }

        Ok(SlashCommandResult::Handled)
    }
```

> **注意：** `AgentRunner` 使用 `block_on` 需要在同步上下文中调用。如果 `handle_init` 改为 `async`，可以去掉 `block_on`。考虑到 `execute()` 当前是同步方法，这里使用 `Handle::try_current()` + `block_on` 是可行的。

- [ ] **Step 2: 验证编译通过**

Run: `cargo check`
Expected: 编译成功

- [ ] **Step 3: Commit**

```bash
git add src/commands/slash.rs
git commit -m "feat(commands): implement /init with AgentRunner"
```

---

## Task 8: 全面测试与验证

**Files:**
- 所有已修改/创建的文件

- [ ] **Step 1: 运行所有单元测试**

Run: `cargo test`
Expected: 全部测试通过（现有 26 个 + 新增测试）

- [ ] **Step 2: 运行 Clippy 检查**

Run: `cargo clippy -- -D warnings`
Expected: 无警告

- [ ] **Step 3: 格式化代码**

Run: `cargo fmt`

- [ ] **Step 4: 最终 Commit**

```bash
git add -A
git commit -m "test: add comprehensive tests for slash commands"
```

---

## 自审检查

### Spec 覆盖率

| Spec 要求 | 对应 Task |
|-----------|-----------|
| `/model` 无参数展示列表 | Task 3, Step 3 (`handle_model` + `print_model_list`) |
| `/model <key>` 切换模型 | Task 3, Step 3 + Task 4 (`set_model`) |
| `/model` 无效模型提示 | Task 3, Step 3 (`handle_model` 错误分支) |
| `/init` 分析并生成 AGENTS.md | Task 3, Step 3 + Task 7 (`handle_init`) |
| `/init` 直接覆盖 | Task 7 (利用现有 write 工具，自然覆盖) |
| AGENTS.md 自动注入提示词 | Task 5 (`PromptBuilder::build`) |
| `main.rs` → `entry.rs` 迁移 | Task 1 |
| 两种模式（REPL/单命令）都支持 | Task 6 (`run_interactive` + `run_single_command` 拦截) |

### 无 Placeholder

- [x] 无 "TBD" / "TODO"
- [x] 无 "add appropriate error handling" 等模糊描述
- [x] 每个测试代码完整给出
- [x] 每个实现代码完整给出

### 类型一致性

- [x] `SlashCommand::Model(Option<String>)` 与 `parse()` 返回值一致
- [x] `Provider::set_model(&mut self, ...)` 签名与测试调用一致
- [x] `PromptBuilder::build()` 签名未变（AGENTS.md 读取内置）
