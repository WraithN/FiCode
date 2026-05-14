# 重构记录：预设模型与预设主题 JSON 化

**处理时间**：2026-05-14 21:15
**模块**：`crates/core/src/config/presets.rs`、`crates/shared/src/dto.rs`
**相关 Commit**：(待填充)

---

## 重构动机

预设模型和预设主题原本以硬编码 Rust 结构体的形式存在：

- **预设模型**：`crates/core/src/config/presets.rs` 中 `default_providers()` 函数，约 420 行 `HashMap::insert` + 结构体初始化代码
- **预设主题**：`crates/shared/src/dto.rs` 中 `ThemePreset::all_presets()` 方法，约 640 行 `vec![ThemePreset { ... }]` 代码

问题：
1. 数据与代码混杂，难以快速查看或修改预设内容
2. 非 Rust 开发者（如产品经理、设计师）无法直接编辑
3. 代码冗长，阅读和维护成本高

---

## 具体改动

### 1. 新增 `preset_models.json`

文件：`crates/core/src/config/preset_models.json`

将 6 个预设 Provider（openai、glm、kimi、qwen、deepseek、anthropic）及其模型配置提取为 JSON：

```json
{
  "openai": {
    "provider_type": "open_ai_compatible",
    "npm": "@ai-sdk/openai",
    "name": "OpenAI",
    "options": { "apiKey": "", "baseURL": "https://api.openai.com/v1", ... },
    "models": { "gpt-4o": { "name": "GPT-4o", "limit": { "context": 128000, "output": 16384 } }, ... }
  },
  ...
}
```

### 2. 新增 `preset_themes.json`

文件：`crates/shared/src/preset_themes.json`

将 29 个内置主题（deep_ocean、github_dark、dracula、nord、catppuccin_mocha 等）提取为 JSON 数组：

```json
[
  { "name": "deep_ocean", "description": "Deep Ocean Dark", "bg_base": 854221, ... },
  ...
]
```

### 3. 重构 `presets.rs`

原 `default_providers()`：逐一手动构建 `HashMap<String, ProviderConfig>`

新 `default_providers()`：

```rust
const PRESET_MODELS_JSON: &str = include_str!("preset_models.json");

pub fn default_providers() -> HashMap<String, ProviderConfig> {
    serde_json::from_str(PRESET_MODELS_JSON)
        .expect("preset_models.json 格式错误")
}
```

删除了约 320 行硬编码的 Provider/Model 初始化代码，保留 `merge_presets()` 合并逻辑和单元测试。

### 4. 重构 `dto.rs`

原 `ThemePreset::all_presets()`：硬编码 `vec![ThemePreset { ... }]` 共 29 个元素

新 `ThemePreset::all_presets()`：

```rust
const PRESET_THEMES_JSON: &str = include_str!("preset_themes.json");

impl ThemePreset {
    pub fn all_presets() -> Vec<Self> {
        serde_json::from_str(PRESET_THEMES_JSON)
            .expect("preset_themes.json 格式错误")
    }
}
```

删除了约 640 行硬编码的主题初始化代码。

---

## 预期收益

1. **数据与代码分离**：预设内容独立在 JSON 文件中，无需修改 Rust 代码即可调整
2. **可读性提升**：JSON 格式比 Rust 结构体初始化更紧凑、更易读
3. **降低维护成本**：新增/修改预设只需编辑 JSON，无需重新理解 Rust 代码结构
4. **编译时安全**：`include_str!` 保证 JSON 文件存在，编译失败比运行时 panic 更安全
5. **代码瘦身**：`presets.rs` 从 454 行减至约 110 行，`dto.rs` 从 1085 行减至约 450 行

---

## 验证

- `cargo build --workspace`：编译成功，0 错误，0 警告
- `cargo test --workspace`：全部 249 个测试通过，0 失败
- 预设合并逻辑测试、主题相关测试均通过
