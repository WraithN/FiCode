# Config Module Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a unified config module with JSON/JSONC file support, hot-reload via filesystem events, env var placeholders, and integration into Provider + CLI.

**Architecture:** Explicit dependency injection via `Arc<RwLock<Config>>` shared across main.rs and Provider. Filesystem watcher with 500ms debounce for hot-reload. Environment variables take precedence over config file.

**Tech Stack:** Rust, serde, notify 8.x, jsonc-parser 0.32

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `Cargo.toml` | Modify | Add `notify` and `jsonc-parser` dependencies |
| `src/config/mod.rs` | Create | Module entry, re-exports, unit tests |
| `src/config/models.rs` | Create | Config structs: `Config`, `ProviderConfig`, `ProviderOptions`, `ModelConfig`, `ModelLimits` |
| `src/config/config.rs` | Create | `Config::load()`, `Config::config_dir()`, `spawn_watcher()`, env placeholder resolution |
| `src/utils/cli.rs` | Modify | Add `-m/--models` CLI flag |
| `src/provider/provider.rs` | Modify | `Provider::new()` accepts `Arc<RwLock<Config>>`, `from_env()`, `from_config()` |
| `src/main.rs` | Modify | Add `mod config;`, init config + watcher, handle `--models`, pass config to Provider |

---

### Task 1: Add Dependencies

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Add `notify` and `jsonc-parser` to dependencies**

```toml
[dependencies]
# ... existing deps ...
notify = "8"
jsonc-parser = "0.32"
```

- [ ] **Step 2: Verify dependencies resolve**

Run: `cargo check`
Expected: Compiles successfully (no new code yet, just deps)

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "deps: add notify and jsonc-parser for config hot-reload"
```

---

### Task 2: Create Config Data Models

**Files:**
- Create: `src/config/models.rs`

- [ ] **Step 1: Create the file with all config structs**

```rust
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct Config {
    pub model: String,
    pub provider: HashMap<String, ProviderConfig>,
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
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check`
Expected: Compiles (file not linked yet, but syntax ok)

- [ ] **Step 3: Commit**

```bash
git add src/config/models.rs
git commit -m "feat(config): add config data models"
```

---

### Task 3: Implement Config Loading and Parsing

**Files:**
- Create: `src/config/config.rs`

- [ ] **Step 1: Write config loading implementation**

```rust
use anyhow::{anyhow, Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use super::models::{Config, ProviderConfig, ProviderOptions, ModelConfig, ModelLimits};

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

    fn parse(content: &str, is_jsonc: bool) -> Result<Self> {
        let json_str = if is_jsonc {
            strip_jsonc_comments(content)?
        } else {
            content.to_string()
        };

        let mut config: Config = serde_json::from_str(&json_str)
            .with_context(|| "配置文件格式错误")?;
        config.resolve_env_placeholders()?;
        Ok(config)
    }

    fn resolve_env_placeholders(&mut self) -> Result<()> {
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

fn strip_jsonc_comments(content: &str) -> Result<String> {
    use jsonc_parser::parse_to_value;
    use jsonc_parser::CollectOptions;

    let options = CollectOptions {
        comments: true,
        tokens: false,
    };

    let value = parse_to_value(content, &options)
        .map_err(|e| anyhow!("JSONC 解析失败: {:?}", e))?;

    match value {
        Some(v) => Ok(v.to_string()),
        None => Ok("{}".to_string()),
    }
}

fn extract_env_var(placeholder: &str) -> Result<String> {
    let start = placeholder.find("{env:").ok_or_else(|| anyhow!("无效的环境变量占位符"))? + 5;
    let end = placeholder.find('}').ok_or_else(|| anyhow!("占位符缺少闭合括号"))?;
    Ok(placeholder[start..end].to_string())
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check`
Expected: Compiles (file not linked yet)

- [ ] **Step 3: Commit**

```bash
git add src/config/config.rs
git commit -m "feat(config): implement config loading with JSONC and env placeholder support"
```

---

### Task 4: Create Config Module Entry with Tests

**Files:**
- Create: `src/config/mod.rs`

- [ ] **Step 1: Create module entry**

```rust
pub mod config;
pub mod models;

pub use models::{Config, ModelConfig, ModelLimits, ProviderConfig, ProviderOptions};
pub use config::Config as ConfigLoader;
```

- [ ] **Step 2: Add unit tests**

Append to `src/config/mod.rs`:

```rust
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

        let config = config::Config::parse(json, false).unwrap();
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

        let config = config::Config::parse(jsonc, true).unwrap();
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

        let mut config = config::Config::parse(json, false).unwrap();
        config.resolve_env_placeholders().unwrap();
        
        let provider = config.provider.get("test").unwrap();
        assert_eq!(provider.options.api_key, "resolved-key");
    }
}
```

Note: `Config::parse` and `resolve_env_placeholders` need to be `pub(crate)` in `src/config/config.rs` for tests to access them.

- [ ] **Step 3: Make test helpers accessible**

In `src/config/config.rs`, change:
```rust
pub fn parse(content: &str, is_jsonc: bool) -> Result<Self> { ... }
pub fn resolve_env_placeholders(&mut self) -> Result<()> { ... }
```

- [ ] **Step 4: Link module in main.rs**

Add to `src/main.rs` (with other `mod` declarations):
```rust
mod config;
```

- [ ] **Step 5: Run tests**

Run: `cargo test config::tests`
Expected: All 4 tests PASS

- [ ] **Step 6: Commit**

```bash
git add src/config/mod.rs src/config/config.rs src/main.rs
git commit -m "feat(config): add config module entry and unit tests"
```

---

### Task 5: Implement Hot-Reload Watcher with Debounce

**Files:**
- Modify: `src/config/config.rs`

- [ ] **Step 1: Add hot-reload watcher implementation**

Append to `src/config/config.rs`:

```rust
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};
use notify::{Event, RecursiveMode, Watcher};

pub fn spawn_watcher(config: Arc<RwLock<Config>>) -> Result<impl Watcher> {
    let config_dir = Config::config_dir();
    let last_reload = Arc::new(Mutex::new(Instant::now()));

    let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
        let Ok(event) = res else { return };
        if !event.kind.is_modify() { return };

        let mut last = last_reload.lock().unwrap();
        if last.elapsed() < Duration::from_millis(500) {
            return;
        }
        *last = Instant::now();
        drop(last);

        let Ok(new_config) = Config::load() else {
            log_warn!("配置热重载失败");
            return;
        };

        let Ok(mut cfg) = config.write() else {
            log_warn!("配置锁中毒，无法更新");
            return;
        };

        *cfg = new_config;
        log_info!("配置已热重载");
    })?;

    watcher.watch(&config_dir, RecursiveMode::NonRecursive)?;
    Ok(watcher)
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check`
Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
git add src/config/config.rs
git commit -m "feat(config): add hot-reload watcher with 500ms debounce"
```

---

### Task 6: Add CLI `--models` Flag

**Files:**
- Modify: `src/utils/cli.rs`

- [ ] **Step 1: Add `-m/--models` argument**

```rust
/// Show configured providers and models
#[arg(short = 'm', long = "models")]
pub models: bool,
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check`
Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
git add src/utils/cli.rs
git commit -m "feat(cli): add -m/--models flag to show configured models"
```

---

### Task 7: Integrate Config into Provider

**Files:**
- Modify: `src/provider/provider.rs`

- [ ] **Step 1: Update imports and ModelType detection**

Add to imports:
```rust
use std::sync::{Arc, RwLock};
use crate::config::{Config, ProviderConfig};
```

- [ ] **Step 2: Rewrite Provider::new()**

```rust
impl Provider {
    pub fn new(config: Arc<RwLock<Config>>) -> Result<Self> {
        // 1. 优先尝试环境变量
        if let Ok(model) = Self::from_env() {
            return Ok(Self { model: Some(model) });
        }

        // 2. 降级到配置文件
        let cfg = config.read().map_err(|_| anyhow!("配置锁中毒"))?;
        let model = Self::from_config(&cfg)?;
        Ok(Self { model: Some(model) })
    }

    fn from_env() -> Result<Model> {
        dotenvy::dotenv().ok();

        if let (Ok(api_key), Ok(base_url), Ok(model_name)) = (
            std::env::var("OPENAI_API_KEY"),
            std::env::var("OPENAI_BASE_URL"),
            std::env::var("OPENAI_MODEL"),
        ) {
            return Ok(Model {
                api_key,
                base_url,
                model_name,
                model_type: ModelType::OpenAiCompatible,
            });
        }

        let anthropic_api_key =
            std::env::var("ANTHROPIC_API_KEY").or_else(|_| std::env::var("ANTHROPIC_AUTH_TOKEN"));
        if let (Ok(api_key), Ok(base_url), Ok(model_name)) = (
            anthropic_api_key,
            std::env::var("ANTHROPIC_BASE_URL"),
            std::env::var("ANTHROPIC_MODEL"),
        ) {
            return Ok(Model {
                api_key,
                base_url,
                model_name,
                model_type: ModelType::Anthropic,
            });
        }

        Err(anyhow!("未找到环境变量配置"))
    }

    fn from_config(config: &Config) -> Result<Model> {
        for (provider_name, provider_cfg) in &config.provider {
            if provider_cfg.models.contains_key(&config.model) {
                let model_type = match provider_name.as_str() {
                    "anthropic" => ModelType::Anthropic,
                    _ => ModelType::OpenAiCompatible,
                };
                return Ok(Model {
                    api_key: provider_cfg.options.api_key.clone(),
                    base_url: provider_cfg.options.base_url.clone(),
                    model_name: config.model.clone(),
                    model_type,
                });
            }
        }

        Err(anyhow!(
            "默认模型 '{}' 在配置中未找到",
            config.model
        ))
    }
    // ... rest of Provider impl (model_name, get_client) stays unchanged
}
```

- [ ] **Step 3: Update tests**

In the `#[cfg(test)]` block, the existing tests create `Provider { model: Some(...) }` directly, so they don't need changes. But add a new test for config-based provider:

```rust
#[test]
fn test_provider_from_config() {
    use std::collections::HashMap;
    use crate::config::models::{Config, ProviderConfig, ProviderOptions, ModelConfig, ModelLimits};

    let mut provider_map = HashMap::new();
    provider_map.insert("openai".to_string(), ProviderConfig {
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
            m.insert("gpt-4".to_string(), ModelConfig {
                name: "GPT-4".to_string(),
                limit: ModelLimits { context: 128000, output: 4096 },
            });
            m
        },
    });

    let config = Config {
        model: "gpt-4".to_string(),
        provider: provider_map,
    };

    let model = Provider::from_config(&config).unwrap();
    assert_eq!(model.model_name, "gpt-4");
    assert_eq!(model.api_key, "test-key");
    assert_eq!(model.model_type, ModelType::OpenAiCompatible);
}
```

Note: `from_config` needs to be `pub(crate)` or `#[cfg(test)] pub` for this test to work.

- [ ] **Step 4: Run tests**

Run: `cargo test provider::`
Expected: Existing tests still pass, new test passes

- [ ] **Step 5: Commit**

```bash
git add src/provider/provider.rs
git commit -m "feat(provider): integrate config with env var priority"
```

---

### Task 8: Wire Everything Together in main.rs

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Add `mod config;` and imports**

Add with other mod declarations:
```rust
mod config;
```

Add to imports:
```rust
use config::Config;
use std::sync::{Arc, RwLock};
```

- [ ] **Step 2: Initialize config and watcher early in main()**

After `let args = Args::parse();`:

```rust
let config = Arc::new(RwLock::new(Config::load()?));
let _watcher = config::spawn_watcher(Arc::clone(&config))?;
```

- [ ] **Step 3: Add `--models` handler**

After config initialization, before provider creation:

```rust
if args.models {
    let cfg = config.read().map_err(|_| anyhow!("配置锁中毒"))?;
    println!("Providers and Models:");
    for (provider_key, provider_cfg) in &cfg.provider {
        println!("  {} ({})", provider_key, provider_cfg.name);
        for (model_key, model_cfg) in &provider_cfg.models {
            println!("    {} — {}", model_key, model_cfg.name);
            println!("      context: {}, output: {}", 
                model_cfg.limit.context, model_cfg.limit.output);
        }
    }
    return Ok(());
}
```

- [ ] **Step 4: Pass config to Provider**

Change:
```rust
let provider = Provider::new(Arc::clone(&config))?;
```

- [ ] **Step 5: Verify compilation**

Run: `cargo check`
Expected: Compiles successfully

- [ ] **Step 6: Run full test suite**

Run: `cargo test`
Expected: All tests pass

- [ ] **Step 7: Commit**

```bash
git add src/main.rs
git commit -m "feat(main): integrate config module, hot-reload, and --models flag"
```

---

### Task 9: End-to-End Verification

**Files:**
- None (uses test config file)

- [ ] **Step 1: Create test config directory and file**

```bash
mkdir -p ~/.config/shun-code
cat > ~/.config/shun-code/config.json << 'EOF'
{
  "model": "my-model",
  "provider": {
    "openai": {
      "npm": "@ai-sdk/openai-compatible",
      "name": "OpenAI Compatible",
      "options": {
        "apiKey": "sk-test-key",
        "baseURL": "https://api.openai.com/v1",
        "timeout": 300000,
        "chunkTimeout": 10000
      },
      "models": {
        "my-model": {
          "name": "My Test Model",
          "limit": { "context": 200000, "output": 65536 }
        }
      }
    }
  }
}
EOF
```

- [ ] **Step 2: Test `--models` output**

Run: `cargo run -- --models`
Expected output:
```
Providers and Models:
  openai (OpenAI Compatible)
    my-model — My Test Model
      context: 200000, output: 65536
```

- [ ] **Step 3: Test with env var priority**

Run: `OPENAI_API_KEY=real-key OPENAI_BASE_URL=https://real.com OPENAI_MODEL=gpt-4 cargo run -- --models`
Expected: `--models` still shows config models (env vars affect Provider, not `--models` display)

Actually, verify Provider picks up env vars by checking interactive mode starts without error:
Run: `OPENAI_API_KEY=real-key OPENAI_BASE_URL=https://real.com OPENAI_MODEL=gpt-4 cargo run -- -c "hello"`
Expected: Runs successfully (or fails at API call, not at config loading)

- [ ] **Step 4: Clean up test config (optional)**

```bash
rm ~/.config/shun-code/config.json
```

- [ ] **Step 5: Final commit**

```bash
git commit --allow-empty -m "test: verify config module end-to-end"
```

---

## Self-Review

**Spec coverage:**
- ✅ Config file path (`~/.config/shun-code/config.json/c`) → Task 3
- ✅ Hot reload → Task 5
- ✅ JSONC support → Task 3
- ✅ Env var placeholders → Task 3
- ✅ Provider integration with env priority → Task 7
- ✅ CLI `-m/--models` → Task 6, Task 8
- ✅ Config format with model/provider/models → Task 2

**Placeholder scan:**
- ✅ No TBD/TODO/fill-in-details
- ✅ All code is complete and copy-paste ready

**Type consistency:**
- ✅ `Config` used consistently across all tasks
- ✅ `Arc<RwLock<Config>>` pattern consistent in Task 5, 7, 8
