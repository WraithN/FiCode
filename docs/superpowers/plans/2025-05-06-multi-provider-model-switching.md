# Multi-Provider Model Switching Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use subagent-driven-development or executing-plans to implement this plan task-by-task.

**Goal:** Add built-in preset providers (glm, kimi, qwen, openai, anthropic) with two-level model switching (Provider → Model) and inline api_key prompting in TUI.

**Architecture:** Extend Config/Provider with explicit `provider_type` and preset defaults. Add two-phase TUI submenu (SubmenuKind::Model). Extend server APIs for provider-grouped model listing and switching.

**Tech Stack:** Rust (tokio, ratatui, serde_json, reqwest)

---

## File Map

| File | Responsibility |
|------|---------------|
| `src/config/models.rs` | Add `ProviderType` enum, extend `ProviderConfig` |
| `src/config/config.rs` | Preset provider merge logic at config load time |
| `src/provider/provider.rs` | `ProviderType`-based model detection, env var prefixes, `set_model` with api_key override |
| `src/provider/mod.rs` | Re-export `ProviderType` |
| `src/server/api/chat_api.rs` | Add `/api/model/switch` endpoint |
| `src/server/transport/rpc.rs` | Update `list_models` to return provider-grouped data |
| `src/server/commands.rs` | Update `/models` command handler for two-phase + metadata |
| `src/tui/components/input.rs` | Add `SubmenuKind::Model`, two-phase selection state |
| `src/tui/components/header.rs` | Display `provider / model` format, populate models from server |
| `src/tui/client.rs` | Add `list_models()` and `switch_model()` methods |
| `src/tui/app.rs` | Wire `AppEvent::SelectModel` to call server switch |

---

## Tasks

### Phase 1: Config + Preset Providers

- [ ] **Task 1.1**: Add `ProviderType` enum to `src/config/models.rs`
  - Variants: `OpenAiCompatible`, `Anthropic`
  - Derive `Debug, Clone, Serialize, Deserialize`

- [ ] **Task 1.2**: Extend `ProviderConfig` with `provider_type` field
  - Add `provider_type: ProviderType` to struct
  - Default to `OpenAiCompatible` for backward compat

- [ ] **Task 1.3**: Add preset provider defaults function
  - Create `src/config/presets.rs` with `default_providers() -> HashMap<String, ProviderConfig>`
  - Define presets: openai, glm, kimi, qwen, anthropic
  - Each preset has base_url, type, and default models

- [ ] **Task 1.4**: Merge preset providers at config load time
  - In `Config::load()`, after parsing user config, merge presets
  - User config overrides preset fields (api_key, models, etc.)
  - Custom providers (not in presets) are preserved as-is

- [ ] **Task 1.5**: Write config tests
  - Test: Preset provider merged when not in user config
  - Test: User config overrides preset base_url
  - Test: Custom provider preserved alongside presets
  - Run `cargo test config::` to verify

### Phase 2: Provider Runtime

- [ ] **Task 2.1**: Update `ModelType` detection
  - In `Provider::from_config()`, use `provider_cfg.provider_type` instead of string match
  - Update `Provider::set_model()` to read `provider_type` from config

- [ ] **Task 2.2**: Add provider-specific env var support
  - Extend `Provider::from_env()` to check `PROVIDER_NAME_API_KEY` pattern
  - Priority: `GLM_API_KEY` > `OPENAI_API_KEY` for glm provider
  - Map: openai→OPENAI, glm→GLM, kimi→KIMI, qwen→QWEN/DASHSCOPE, anthropic→ANTHROPIC

- [ ] **Task 2.3**: Extend `set_model` with api_key override
  - Signature: `set_model(&mut self, model_name: &str, config: &Config, api_key: Option<&str>) -> Result<()>`
  - If `api_key` is Some, use it instead of config's api_key
  - Update all call sites (CLI, server commands, tests)

- [ ] **Task 2.4**: Write provider tests
  - Test: `set_model` with api_key override uses provided key
  - Test: Env var resolution for each preset provider
  - Test: `ProviderType::Anthropic` detection from config
  - Run `cargo test provider::` to verify

### Phase 3: Server API

- [ ] **Task 3.1**: Update JSON-RPC `list_models`
  - Return provider-grouped structure:
    ```json
    { "providers": [{ "key": "kimi", "name": "Moonshot", "type": "openai_compatible", "models": [...] }] }
    ```
  - Update `handle_list_models` in `rpc.rs`

- [ ] **Task 3.2**: Add `/api/model/switch` endpoint
  - POST body: `{ "provider": "kimi", "model": "moonshot-v1-8k", "api_key": "sk-xxx" }`
  - Calls `provider.set_model()` with optional api_key
  - Returns `{ "provider": "kimi", "model": "moonshot-v1-8k" }`
  - Add to `chat_api.rs`

- [ ] **Task 3.3**: Update `/models` command
  - Support `/models <provider>/<model>` syntax
  - Support `/models <provider>/<model> --key <api_key>`
  - Return metadata: `current_model`, `provider`, `provider_type`
  - Update `commands::slash.rs` handler

- [ ] **Task 3.4**: Write server tests
  - Test: `list_models` returns provider-grouped data
  - Test: `/api/model/switch` with valid provider/model
  - Test: `/api/model/switch` with api_key override
  - Run `cargo test server::` to verify

### Phase 4: TUI Model Submenu

- [ ] **Task 4.1**: Add `SubmenuKind::Model` to Input component
  - Add variant to enum in `input.rs`
  - Add model submenu state struct with two-phase selection

- [ ] **Task 4.2**: Implement provider list rendering
  - Fetch provider list from `client.list_models()`
  - Render provider names with `▶` indicator
  - Support ↑/↓ navigation, Enter to select

- [ ] **Task 4.3**: Implement model list rendering (phase 2)
  - After provider selected, show models for that provider
  - Support ↑/↓, Enter to select model

- [ ] **Task 4.4**: Implement api_key prompt (phase 3)
  - After model selected, check if api_key configured
  - If missing, switch input to api_key mode (masked input)
  - On Enter, proceed to server switch call

- [ ] **Task 4.5**: Wire Ctrl+M to Model submenu
  - In `app.rs`, `Ctrl+M` triggers `Input::open_model_submenu()`
  - Handle `AppEvent::OpenModelSubmenu` (if needed)

- [ ] **Task 4.6**: Wire selection to server call
  - On model selected (+ api_key if needed), call `client.switch_model()`
  - On success, update `Header.current_model` to `"provider / model"`
  - Add system message to chat: "✅ 已切换模型: provider / model"

- [ ] **Task 4.7**: Write TUI tests
  - Test: SubmenuKind::Model state transitions (Provider → Model → ApiKey)
  - Test: Model selection event handling
  - Run `cargo test tui::components::input::` to verify

### Phase 5: Header Integration + Polish

- [ ] **Task 5.1**: Update Header display format
  - Show `provider / model` instead of just model name
  - Update `header.rs` rendering

- [ ] **Task 5.2**: Populate models at startup
  - In `TuiApp::run()`, call `client.list_models().await`
  - Cache provider-grouped model list in app state
  - Pass to `Input` component for submenu rendering

- [ ] **Task 5.3**: Update existing `/models` slash command in TUI
  - When server returns model switch result with metadata
  - Update header and chat immediately

- [ ] **Task 5.4**: Full integration test
  - Start TUI, press Ctrl+M, select provider, select model, input api_key
  - Verify header updates, chat shows success message
  - Verify subsequent chat uses new model

- [ ] **Task 5.5**: Run full test suite
  - `cargo test` — all 111+ tests should pass
  - `cargo clippy` — no warnings
  - `cargo fmt` — code formatted

---

## Testing Strategy

1. **Unit tests** for each modified module (config, provider, server commands)
2. **Integration tests** for server endpoints (`/api/model/switch`, `list_models`)
3. **Manual TUI tests** for two-phase submenu flow
4. **Regression tests**: Existing model switching (CLI `/models gpt-4`) still works

## Risks

- **Config merge complexity**: Preset + user config merge must not lose user data
- **Env var collision**: Multiple providers might share same env var pattern
- **TUI state management**: Two-phase submenu adds complexity to Input component
- **Backward compat**: Existing `--models` flag and config.json must still work

## Success Criteria

- [ ] `cargo test` passes (111+ tests)
- [ ] TUI Ctrl+M opens provider list
- [ ] Selecting provider shows its models
- [ ] Selecting model with empty api_key prompts for input
- [ ] Switching model updates header to `provider / model`
- [ ] Chat shows success message after switch
- [ ] Custom providers from config appear in provider list
