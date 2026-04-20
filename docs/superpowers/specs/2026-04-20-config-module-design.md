# 配置模块设计文档

## 背景与目标

为 `shun-code` 新增统一的配置模块，支持通过 `~/.config/shun-code/config.json` 或 `config.jsonc` 管理模型和 Provider 设置，并实现热加载、环境变量占位符解析，以及与现有 `provider.rs` 的深度集成。

## 文件结构

```
src/config/
├── mod.rs      # 模块入口，导出 Config 和相关类型
├── config.rs   # Config 结构体、加载逻辑、热加载监听
└── models.rs   # 配置数据类型（ProviderConfig、ModelConfig 等）
```

## 数据模型（models.rs）

```rust
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub model: String,
    pub provider: HashMap<String, ProviderConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProviderConfig {
    pub npm: String,
    pub name: String,
    pub options: ProviderOptions,
    pub models: HashMap<String, ModelConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProviderOptions {
    #[serde(rename = "apiKey")]
    pub api_key: String,
    #[serde(rename = "baseURL")]
    pub base_url: String,
    pub timeout: u64,
    #[serde(rename = "chunkTimeout")]
    pub chunk_timeout: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModelConfig {
    pub name: String,
    pub limit: ModelLimits,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModelLimits {
    pub context: u32,
    pub output: u32,
}
```

## 配置加载机制（config.rs）

### 文件查找顺序

1. `~/.config/shun-code/config.jsonc`
2. `~/.config/shun-code/config.json`
3. 若均不存在，返回 `Config::default()`（空配置）

### JSONC 支持

引入 `jsonc-parser = "0.26"`，在加载 `*.jsonc` 文件时先去除 `//` 和 `/* */` 注释，再交由 `serde_json` 解析。

### 环境变量占位符解析

`api_key` 字段支持 `{env:VAR_NAME}` 语法。加载时自动替换为对应环境变量的值，若变量未设置则返回错误。

```rust
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
```

## 热加载机制

使用 `notify = "6.1"` 监听配置文件目录，通过 `Arc<RwLock<Config>>` 共享配置状态。

**关键优化：**
- **防抖**：500ms 窗口期内忽略重复触发（解决编辑器保存时的多次 `write` 事件）
- **扁平化错误处理**：使用 `let Ok(x) = ... else { return };` 替代深层嵌套

```rust
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use notify::{Event, RecursiveMode, Watcher};

pub fn spawn_watcher(config: Arc<RwLock<Config>>) -> Result<impl Watcher> {
    let config_dir = Config::config_dir();
    let last_reload = Arc::new(Mutex::new(Instant::now()));
    
    let mut watcher = notify::recommended_watcher(move |res: Result<Event, _>| {
        let Ok(event) = res else { return };
        if !event.kind.is_modify() { return };
        
        let mut last = last_reload.lock().unwrap();
        if last.elapsed() < Duration::from_millis(500) { return }
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

## Provider 集成

### 优先级规则

**环境变量 > 配置文件 > 默认值**

`Provider::new()` 改造为接受 `Arc<RwLock<Config>>`：

1. 首先尝试从环境变量构建 `Model`（保持现有 `OPENAI_API_KEY` / `ANTHROPIC_API_KEY` 逻辑不变）
2. 若环境变量不完整，从 `Config` 中查找默认 `model` 对应的 Provider 配置
3. 若均未找到，返回错误

```rust
impl Provider {
    pub fn new(config: Arc<RwLock<Config>>) -> Result<Self> {
        if let Ok(model) = Self::from_env() {
            return Ok(Self { model: Some(model) });
        }
        
        let cfg = config.read().map_err(|_| anyhow!("配置锁中毒"))?;
        let model = Self::from_config(&cfg)?;
        Ok(Self { model: Some(model) })
    }
    
    fn from_env() -> Result<Model> { /* 现有逻辑 */ }
    
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
        Err(anyhow!("默认模型 '{}' 在配置中未找到", config.model))
    }
}
```

## CLI 扩展

### 新增参数

```rust
/// 展示当前配置的所有模型信息
#[arg(short = 'm', long = "models")]
pub models: bool,
```

### 输出格式

```
Providers and Models:
  openai (OpenAI Compatible)
    gpt-4o — GPT-4o
      context: 200000, output: 65536
    gpt-4o-mini — GPT-4o Mini
      context: 128000, output: 4096
  anthropic (Anthropic)
    claude-3-7-sonnet — Claude 3.7 Sonnet
      context: 200000, output: 8192
```

### main.rs 启动流程

```rust
#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    
    // 1. 加载配置
    let config = Arc::new(RwLock::new(Config::load()?));
    
    // 2. 启动热加载监听器
    let _watcher = config::spawn_watcher(Arc::clone(&config))?;
    
    // 3. 处理 --models
    if args.models {
        let cfg = config.read().map_err(|_| anyhow!("配置锁中毒"))?;
        print_models(&cfg);
        return Ok(());
    }
    
    // 4. 初始化 Provider（传入配置）
    let provider = Provider::new(Arc::clone(&config))?;
    ...
}
```

## 错误处理

- 配置文件解析失败：记录警告，继续使用当前内存中的配置（热加载场景）或返回错误（启动场景）
- 环境变量占位符解析失败：启动时直接报错，明确提示缺失的变量名
- 默认模型未找到：Provider::new() 返回清晰的错误信息

## 测试策略

- `Config::load()`：测试 JSON/JSONC 解析、环境变量占位符替换
- `spawn_watcher`：测试防抖逻辑（模拟快速多次文件变更）
- `Provider::new()`：测试优先级规则（环境变量优先于配置文件）
- CLI `--models`：测试输出格式正确性
