# Multi-Provider Model Switching Design

## Overview

Support multiple built-in providers (glm, openai, kimi, qwen3.5) plus custom providers defined in config. Model switching follows a two-level flow: select Provider → select Model → optionally input api_key (skipped if already configured).

## Goals

1. Built-in providers work out of the box with just an api_key
2. Custom providers can be fully defined in `config.json`
3. TUI `/models` (Ctrl+M) opens a two-level submenu: Provider list → Model list
4. If api_key is missing, prompt user to input it inline
5. Server commands and JSON-RPC endpoints support provider-grouped model listing

## Non-Goals

- Provider discovery (auto-fetching model lists from APIs)
- Multiple active providers simultaneously
- Persisting api_key to config file automatically

---

## Configuration Model Changes

### ProviderType Enum

Add an explicit `provider_type` field to `ProviderConfig` to decouple API type detection from provider name string matching:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderType {
    OpenAiCompatible,
    Anthropic,
}
```

### ProviderConfig Extension

```rust
pub struct ProviderConfig {
    pub provider_type: ProviderType,  // NEW: explicit API type
    pub npm: String,
    pub name: String,
    pub options: ProviderOptions,
    pub models: HashMap<String, ModelConfig>,
}
```

### Preset Providers

Built-in defaults (base_url + default models). These are merged with user config at load time, allowing users to override any field:

| Provider | Type | Default Base URL |
|----------|------|-----------------|
| openai | OpenAiCompatible | https://api.openai.com/v1 |
| glm | OpenAiCompatible | https://open.bigmodel.cn/api/paas/v4 |
| kimi | OpenAiCompatible | https://api.moonshot.cn/v1 |
| qwen | OpenAiCompatible | https://dashscope.aliyuncs.com/compatible-mode/v1 |
| anthropic | Anthropic | https://api.anthropic.com |

Preset models (partial list):
- **openai**: gpt-4o, gpt-4o-mini, gpt-4-turbo
- **glm**: glm-4, glm-4-flash, glm-4v
- **kimi**: moonshot-v1-8k, moonshot-v1-32k, moonshot-v1-128k
- **qwen**: qwen-turbo, qwen-plus, qwen-max
- **anthropic**: claude-3-7-sonnet, claude-3-5-sonnet, claude-3-opus

### Config Loading Logic

```
1. Load user config.json
2. For each preset provider:
   - If user config defines this provider → merge (user overrides preset)
   - If not → inject preset with empty api_key
3. Result: Config.provider contains presets + custom providers
```

---

## Provider Runtime Changes

### ModelType Detection

Replace string-matching with `provider_type` field:
```rust
// Before: provider_name == "anthropic"
// After: provider_cfg.provider_type == ProviderType::Anthropic
```

### Environment Variables

Support provider-specific env var prefixes. Priority: `PROVIDER_NAME_API_KEY` > generic `OPENAI_API_KEY`:

| Provider | Env Var |
|----------|---------|
| openai | OPENAI_API_KEY |
| glm | GLM_API_KEY |
| kimi | KIMI_API_KEY |
| qwen | QWEN_API_KEY / DASHSCOPE_API_KEY |
| anthropic | ANTHROPIC_API_KEY |

### Provider::set_model

Accept optional `api_key_override` parameter. If provided, use it instead of config's api_key:
```rust
pub fn set_model(&mut self, model_name: &str, config: &Config, api_key_override: Option<&str>) -> Result<()>
```

---

## TUI Interaction Flow

### Model Switching Submenu (Ctrl+M / `/models`)

```
User presses Ctrl+M
    ↓
Input component opens SubmenuKind::Model
    ↓
Show Provider list (preset + custom)
    ├─ ▼ openai
    ├─ ▶ glm
    ├─ ▶ kimi
    ├─ ▶ qwen
    ├─ ▶ anthropic
    └─ ▶ my-custom-provider
    ↓
User selects "kimi"
    ↓
Show Model list for kimi
    ├─ ▼ moonshot-v1-8k
    ├─ ▶ moonshot-v1-32k
    └─ ▶ moonshot-v1-128k
    ↓
User selects "moonshot-v1-8k"
    ↓
Check if api_key is configured for kimi
    ├─ Yes → Send switch request to server
    └─ No  → Switch input mode to api_key prompt
              User types api_key → Enter → Send switch request
    ↓
Server switches model → Returns success
    ↓
Header updates to "kimi / moonshot-v1-8k"
Chat shows system message: "✅ 已切换模型: kimi / moonshot-v1-8k"
```

### SubmenuKind::Model

Reuse existing `SubmenuKind` infrastructure in `Input` component. Add new variant:
```rust
pub enum SubmenuKind {
    Theme,
    Skill,
    Model,  // NEW
}
```

Two-phase selection state:
```rust
struct ModelSubmenuState {
    phase: ModelSelectPhase,  // Provider | Model | ApiKey
    selected_provider: Option<String>,
    selected_model: Option<String>,
}
```

---

## Server API Changes

### JSON-RPC list_models

Return provider-grouped structure:
```json
{
  "providers": [
    {
      "key": "kimi",
      "name": "Moonshot AI",
      "type": "openai_compatible",
      "models": [
        {"key": "moonshot-v1-8k", "name": "Moonshot v1 8K", "limit": {"context": 8192, "output": 4096}}
      ]
    }
  ]
}
```

### Command: models

Extend `/models` slash command to support two-phase switching:
- `/models` → opens interactive selection (TUI)
- `/models <provider>/<model>` → direct switch
- `/models <provider>/<model> --key <api_key>` → switch with api_key override

Command output metadata:
```json
{
  "current_model": "moonshot-v1-8k",
  "provider": "kimi",
  "provider_type": "openai_compatible"
}
```

### HTTP API (new endpoint)

`POST /api/model/switch`
```json
{
  "provider": "kimi",
  "model": "moonshot-v1-8k",
  "api_key": "sk-xxx"  // optional
}
```

---

## Data Flow

```
TUI App
  ├─ Startup: client.list_models() → populate Header.models
  ├─ Ctrl+M: Input opens Model submenu
  │   └─ Select provider → Select model → (input api_key if needed)
  │   └─ POST /api/model/switch { provider, model, api_key? }
  └─ Header updates current_model display

Server
  ├─ /api/model/switch → Provider::set_model(model, config, api_key?)
  ├─ Provider updates model + api_key in runtime
  └─ Returns current provider + model

Config (persistent)
  ├─ Preset providers merged at load time
  └─ Custom providers from user config.json
```

---

## Error Handling

| Scenario | Behavior |
|----------|----------|
| api_key empty and not provided | Prompt user inline in TUI |
| Invalid api_key | Show error in chat, keep previous model active |
| Model not found in provider | Show error, keep previous model active |
| Provider config missing | Skip from provider list |
| Network error during switch | Show error, keep previous model active |

---

## Testing Strategy

1. **Unit tests**: Config merge logic (preset + user override)
2. **Unit tests**: Provider::set_model with api_key override
3. **Unit tests**: Env var resolution for each preset provider
4. **Integration tests**: Server `/api/model/switch` endpoint
5. **Manual tests**: TUI submenu navigation (Provider → Model → ApiKey)

---

## Implementation Phases

1. **Phase 1**: Config model + preset provider defaults
2. **Phase 2**: Provider runtime changes (provider_type, env vars, set_model api_key)
3. **Phase 3**: Server API changes (grouped list_models, switch endpoint)
4. **Phase 4**: TUI Model submenu (two-phase selection + api_key prompt)
5. **Phase 5**: Slash command updates + Header integration
