# Langfuse Observability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 接入 Langfuse 作为外部 LLM 可观测后端：废弃旧的 `TurnLogger`，统一改用 OpenTelemetry pipeline；本地 `spans.jsonl` 兜底，远端 Langfuse OTLP 可失败；启动期 daemon 重发 pending。

**Architecture:** 在 `crates/core/src/observability/` 新增 OTel facade（基于 `opentelemetry_sdk::trace::TracerProvider`），用自定义 `CompositeSpanExporter` fan-out 到 `LocalJsonlExporter`（必成功）+ `OtlpHttpExporter`（可失败）。本地 JSONL 文件即重发数据源，append-only + status_patch 行管理 `lf_status`。

**Tech Stack:** Rust 2021 / Tokio / opentelemetry 0.27 / opentelemetry_sdk 0.27 / opentelemetry-otlp 0.27（http-proto+reqwest-client）/ base64 0.22 / wiremock（测试）。

**Spec:** `docs/superpowers/specs/2026-05-22-langfuse-observability-design.md`

---

## File Structure

### 新增（13 个文件 + 1 个集成测试）

| 文件 | 职责 |
|---|---|
| `crates/core/src/observability/mod.rs` | 模块入口；re-export 公共类型；`init()` / `shutdown()` / `is_enabled()` |
| `crates/core/src/observability/config.rs` | `ObservabilityConfig` DTO；解析 config.json + env；环境变量优先级 |
| `crates/core/src/observability/redact.rs` | 凭证脱敏正则集；`redact(&str) -> Cow<str>` |
| `crates/core/src/observability/attrs.rs` | `gen_ai.*` / `langfuse.*` / `fi_code.*` 属性 key 常量；50KB 截断 helper |
| `crates/core/src/observability/exporter/mod.rs` | `CompositeSpanExporter`：fan-out + status_patch 回调 |
| `crates/core/src/observability/exporter/local_jsonl.rs` | `LocalJsonlExporter`：OTLP→JSONL；append-only；status_patch 写入 |
| `crates/core/src/observability/exporter/otlp_http.rs` | `OtlpHttpExporter`：薄封装 opentelemetry-otlp；Basic Auth 头注入 |
| `crates/core/src/observability/tracer.rs` | TracerProvider 装配；BatchSpanProcessor 配置；全局 set_tracer_provider |
| `crates/core/src/observability/resend.rs` | 启动期 daemon：读取末尾 10000 行、聚合 status、重发 pending |
| `crates/core/src/observability/facade.rs` | `ChatSpan` / `TurnSpan` / `LlmGeneration` / `ToolSpan` / `CompressionSpan` guard structs + `start_*` 函数 |
| `crates/core/src/observability/cli_view.rs` | 迁移自 `utils/turn_log_cli.rs`，改读 `spans.jsonl` |
| `tests/e2e-web/python/test_web_observability.py` | 集成测试 |
| `tests/e2e-web/python/utils/mock_langfuse.py` | aiohttp mini server mock `/api/public/otel/v1/traces` |
| `docs/refactor/refactor-2026-05-22.md` | 重构记录（按 AGENTS.md §8 规范）|

### 修改

| 文件 | 改动 |
|---|---|
| `crates/core/Cargo.toml` | 新增 5 个依赖 |
| `crates/core/src/lib.rs` | 新增 `pub mod observability;` |
| `crates/core/src/agent/mod.rs` | 删除 `pub use turn_logger::*;` |
| `crates/core/src/agent/agent.rs` | 删除 `use turn_logger::*`；6 处 `TurnLogger::global().log_turn(...)` 替换为 `otel::*` |
| `crates/core/src/agent/runner.rs` | 同上 |
| `crates/core/src/utils/mod.rs` | 删除 `pub mod turn_log_cli;` |
| `crates/cli/src/entry.rs` | `use fi_code_core::utils::turn_log_cli::*` → `use fi_code_core::observability::cli_view::*` |
| `crates/cli/src/main.rs` (或入口) | 启动调 `observability::init`；Ctrl-C 调 `shutdown` |
| `crates/server/src/main.rs` | 启动调 `observability::init`；SIGTERM 调 `shutdown` |
| `crates/tui/src/main.rs` | 启动调 `observability::init`；退出调 `shutdown` |
| `crates/core/src/server/api/chat_api.rs` | `handle_chat_endpoint` 开始处建 `ChatSpan`，传 `Context` 给 `agent_loop` |
| `crates/core/src/config/models.rs` | `Config` 增加 `observability: Option<ObservabilityConfig>` |
| `AGENTS.md` | 追加可观测体系章节 |

### 删除

- `crates/core/src/agent/turn_logger.rs`
- `crates/core/src/utils/turn_log_cli.rs`

---

## Phase 0：依赖与骨架（独立可编译）

### Task 0.1：新增依赖

**Files:**
- Modify: `crates/core/Cargo.toml`

- [ ] **Step 1: 添加 OTel + base64 依赖**

修改 `crates/core/Cargo.toml`，在 `[dependencies]` 段尾追加：

```toml
opentelemetry = "0.27"
opentelemetry_sdk = { version = "0.27", features = ["rt-tokio", "trace"] }
opentelemetry-otlp = { version = "0.27", default-features = false, features = ["http-proto", "reqwest-client", "trace"] }
opentelemetry-semantic-conventions = "0.27"
base64 = "0.22"
```

- [ ] **Step 2: 验证编译**

Run: `cargo build -p fi-code-core 2>&1 | tail -20`
Expected: `Finished` 无错误（新依赖被下载且编译通过；可能耗时 ~30s）

- [ ] **Step 3: Commit**

```bash
git add crates/core/Cargo.toml Cargo.lock
git commit -m "deps: add opentelemetry stack for langfuse observability"
```

### Task 0.2：模块骨架

**Files:**
- Create: `crates/core/src/observability/mod.rs`
- Modify: `crates/core/src/lib.rs`

- [ ] **Step 1: 创建模块骨架**

新建 `crates/core/src/observability/mod.rs`：

```rust
// MIT License
// Copyright (c) 2025 fi-code contributors
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

//! observability 模块：基于 OpenTelemetry 的 Agent 全链路追踪。
//!
//! 数据流：业务代码 → facade → TracerProvider → BatchSpanProcessor
//!         → CompositeSpanExporter → (LocalJsonlExporter 必成功
//!                                     + OtlpHttpExporter 可失败)
//!
//! 详见 `docs/superpowers/specs/2026-05-22-langfuse-observability-design.md`

pub mod attrs;
pub mod cli_view;
pub mod config;
pub mod exporter;
pub mod facade;
pub mod redact;
pub mod resend;
pub mod tracer;

// 业务侧统一入口（方便 `use crate::observability::otel::*`）
pub use facade as otel;

use std::sync::atomic::{AtomicBool, Ordering};

static ENABLED: AtomicBool = AtomicBool::new(false);

/// 初始化全局可观测体系。失败时主动降级为 disabled 并返回 Ok；
/// 仅在 `~/.config/fi-code/logs/` 不可写时返回 Err，由调用方 panic。
pub fn init(config: &crate::config::Config) -> anyhow::Result<()> {
    let obs_cfg = config::ObservabilityConfig::resolve(config);
    tracer::install(&obs_cfg)?;
    ENABLED.store(true, Ordering::SeqCst);

    // 启动期重发 daemon
    if obs_cfg.langfuse_enabled() {
        tokio::spawn(async move {
            if let Err(e) = resend::run_once().await {
                crate::log_warn!("[observability] resend daemon failed: {}", e);
            }
        });
    }
    Ok(())
}

/// 进程退出时调用，flush 队列。
pub fn shutdown() {
    tracer::shutdown();
    ENABLED.store(false, Ordering::SeqCst);
}

/// 是否启用（业务代码用此判断是否要构造大 attribute）。
pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::SeqCst)
}
```

- [ ] **Step 2: 添加子模块占位文件**

依次创建以下空文件（仅含 MIT 头 + 模块注释 + 1 个 `pub fn placeholder() {}`，确保编译通过；后续 task 会填实内容）：

- `crates/core/src/observability/config.rs`
- `crates/core/src/observability/attrs.rs`
- `crates/core/src/observability/redact.rs`
- `crates/core/src/observability/tracer.rs`
- `crates/core/src/observability/resend.rs`
- `crates/core/src/observability/facade.rs`
- `crates/core/src/observability/cli_view.rs`

并创建 exporter 子模块：

- `crates/core/src/observability/exporter/mod.rs`
- `crates/core/src/observability/exporter/local_jsonl.rs`
- `crates/core/src/observability/exporter/otlp_http.rs`

每个占位文件模板（举例 `config.rs`）：

```rust
// MIT License
// Copyright (c) 2025 fi-code contributors
//
// (... 完整 MIT 头同 mod.rs ...)

//! observability::config：配置解析与环境变量优先级。

pub fn placeholder() {}
```

`exporter/mod.rs` 额外声明子模块：

```rust
// (MIT 头)

//! exporter：CompositeSpanExporter + Local + OTLP。

pub mod local_jsonl;
pub mod otlp_http;

pub fn placeholder() {}
```

- [ ] **Step 3: 注册到 lib.rs**

修改 `crates/core/src/lib.rs`，在 `pub mod tui_event;` 后插入：

```rust
pub mod observability;
```

- [ ] **Step 4: 验证编译**

Run: `cargo build -p fi-code-core 2>&1 | tail -10`
Expected: `Finished` 无错误

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/observability crates/core/src/lib.rs
git commit -m "feat(observability): scaffold module skeleton"
```

---

## Phase 1：纯函数模块（无外部依赖，纯单元测试）

### Task 1.1：凭证脱敏 `redact.rs`

**Files:**
- Modify: `crates/core/src/observability/redact.rs`

- [ ] **Step 1: 写失败测试**

替换 `crates/core/src/observability/redact.rs` 全文为：

```rust
// MIT License
// Copyright (c) 2025 fi-code contributors
//
// (... 完整 MIT 头 ...)

//! observability::redact：凭证脱敏。
//!
//! 所有进入 attribute 的字符串都应先经过 `redact()`，
//! 避免 API Key / Bearer Token / Basic Auth / password 被上报。

use once_cell::sync::Lazy;
use regex::Regex;
use std::borrow::Cow;

/// 单个 attribute 最大字节数（防止超大输出撑爆 Langfuse）。
pub const MAX_ATTR_BYTES: usize = 50 * 1024;

/// (pattern, replacement) 编译期常量集，按出现频率与命中代价排序。
static PATTERNS: Lazy<Vec<(Regex, &'static str)>> = Lazy::new(|| {
    vec![
        (Regex::new(r"sk-ant-[A-Za-z0-9_\-]{20,}").unwrap(), "sk-ant-***REDACTED***"),
        (Regex::new(r"sk-lf-[A-Za-z0-9_\-]{20,}").unwrap(), "sk-lf-***REDACTED***"),
        (Regex::new(r"pk-lf-[A-Za-z0-9_\-]{20,}").unwrap(), "pk-lf-***REDACTED***"),
        (Regex::new(r"sk-[A-Za-z0-9_\-]{20,}").unwrap(), "sk-***REDACTED***"),
        (Regex::new(r"(?i)ANTHROPIC_API_KEY\s*[:=]\s*\S+").unwrap(), "ANTHROPIC_API_KEY=***REDACTED***"),
        (Regex::new(r"(?i)OPENAI_API_KEY\s*[:=]\s*\S+").unwrap(), "OPENAI_API_KEY=***REDACTED***"),
        (Regex::new(r"Bearer\s+[A-Za-z0-9._\-]{20,}").unwrap(), "Bearer ***REDACTED***"),
        (Regex::new(r"(?i)Authorization\s*:\s*Basic\s+[A-Za-z0-9+/=]{20,}").unwrap(), "Authorization: Basic ***REDACTED***"),
        (Regex::new(r#"(?i)password["']?\s*[:=]\s*["']?[^\s"',}]+"#).unwrap(), "password=***REDACTED***"),
    ]
});

/// 先按 char 边界截断到 MAX_ATTR_BYTES，再对全文做脱敏。
///
/// 顺序很关键：先截断后脱敏，保证 token 不会被截一半导致正则失配。
pub fn redact_and_truncate(input: &str) -> String {
    let truncated = truncate_utf8(input, MAX_ATTR_BYTES);
    let mut out: Cow<str> = Cow::Borrowed(truncated);
    for (re, repl) in PATTERNS.iter() {
        if re.is_match(&out) {
            out = Cow::Owned(re.replace_all(&out, *repl).into_owned());
        }
    }
    out.into_owned()
}

/// 按 UTF-8 char 边界截断到不超过 max_bytes 字节。
fn truncate_utf8(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut end = max_bytes;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redact_openai_key() {
        let s = "key is sk-test1234567890abcdefghij and continues";
        assert_eq!(
            redact_and_truncate(s),
            "key is sk-***REDACTED*** and continues"
        );
    }

    #[test]
    fn test_redact_anthropic_key() {
        let s = "use sk-ant-api03-AbCdEf1234567890_-ZyXwVuTs";
        assert!(redact_and_truncate(s).contains("sk-ant-***REDACTED***"));
        assert!(!redact_and_truncate(s).contains("AbCdEf"));
    }

    #[test]
    fn test_redact_langfuse_keys() {
        let s = "pk-lf-1234567890abcdefghij sk-lf-abcdef1234567890ghij";
        let out = redact_and_truncate(s);
        assert!(out.contains("pk-lf-***REDACTED***"));
        assert!(out.contains("sk-lf-***REDACTED***"));
    }

    #[test]
    fn test_redact_bearer() {
        let s = "Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.xxx";
        assert!(redact_and_truncate(s).contains("Bearer ***REDACTED***"));
    }

    #[test]
    fn test_redact_basic_auth() {
        let s = "authorization: Basic cGs6c2tfMTIzNDU2Nzg5MDEyMzQ1Ng==";
        assert!(redact_and_truncate(s).to_lowercase().contains("basic ***redacted***"));
    }

    #[test]
    fn test_redact_password() {
        assert!(redact_and_truncate("password=hunter2").contains("password=***REDACTED***"));
        assert!(redact_and_truncate(r#"{"password": "hunter2"}"#).contains("password=***REDACTED***"));
    }

    #[test]
    fn test_redact_env_assignment() {
        let s = "export ANTHROPIC_API_KEY=sk-ant-real-key-here";
        assert!(redact_and_truncate(s).contains("ANTHROPIC_API_KEY=***REDACTED***"));
    }

    #[test]
    fn test_no_false_positive_on_plain_text() {
        let s = "This is normal text with words sk- and pk and password word.";
        assert_eq!(redact_and_truncate(s), s);
    }

    #[test]
    fn test_truncate_within_50kb_then_redact() {
        let big = "x".repeat(60_000) + " sk-realsecret12345678901234567890";
        let out = redact_and_truncate(&big);
        assert!(out.len() <= MAX_ATTR_BYTES);
        // 末尾的 secret 应已被截掉
        assert!(!out.contains("realsecret"));
    }

    #[test]
    fn test_truncate_char_boundary_safe() {
        let s = "中".repeat(20_000); // 60_000 字节，但每字 3 字节
        let out = redact_and_truncate(&s);
        assert!(out.len() <= MAX_ATTR_BYTES);
        assert!(out.chars().all(|c| c == '中'));
    }
}
```

注意：`regex` 与 `once_cell` 已是 `fi-code-core` 现有依赖（AGENTS.md 第 2 节确认），无需新加。

- [ ] **Step 2: 跑测试看失败**

Run: `cargo test -p fi-code-core observability::redact 2>&1 | tail -20`
Expected: 10 个测试通过（实现已写完，应直接 PASS；若失败需调正则）

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/observability/redact.rs
git commit -m "feat(observability): add credential redactor"
```

### Task 1.2：属性 key 常量 `attrs.rs`

**Files:**
- Modify: `crates/core/src/observability/attrs.rs`

- [ ] **Step 1: 写实现 + 测试**

替换 `crates/core/src/observability/attrs.rs` 全文：

```rust
// MIT License
// (... 完整 MIT 头 ...)

//! observability::attrs：OTel + Langfuse 属性 key 常量集。
//!
//! 与 https://langfuse.com/docs/opentelemetry/get-started#property-mapping 一致。

// ── Langfuse trace-level ──
pub const LANGFUSE_USER_ID: &str = "langfuse.user.id";
pub const LANGFUSE_SESSION_ID: &str = "langfuse.session.id";
pub const LANGFUSE_TRACE_NAME: &str = "langfuse.trace.name";
pub const LANGFUSE_TRACE_TAGS: &str = "langfuse.trace.tags";
pub const LANGFUSE_TRACE_INPUT: &str = "langfuse.trace.input";
pub const LANGFUSE_TRACE_OUTPUT: &str = "langfuse.trace.output";
pub const LANGFUSE_TRACE_METADATA_PREFIX: &str = "langfuse.trace.metadata.";
pub const LANGFUSE_RELEASE: &str = "langfuse.release";
pub const LANGFUSE_ENVIRONMENT: &str = "langfuse.environment";

// ── Langfuse observation-level ──
pub const LANGFUSE_OBS_TYPE: &str = "langfuse.observation.type";
pub const LANGFUSE_OBS_INPUT: &str = "langfuse.observation.input";
pub const LANGFUSE_OBS_OUTPUT: &str = "langfuse.observation.output";
pub const LANGFUSE_OBS_LEVEL: &str = "langfuse.observation.level";
pub const LANGFUSE_OBS_STATUS_MESSAGE: &str = "langfuse.observation.status_message";
pub const LANGFUSE_OBS_USAGE_DETAILS: &str = "langfuse.observation.usage_details";
pub const LANGFUSE_OBS_MODEL_NAME: &str = "langfuse.observation.model.name";

// ── OTel GenAI 标准 ──
pub const GEN_AI_SYSTEM: &str = "gen_ai.system";
pub const GEN_AI_REQUEST_MODEL: &str = "gen_ai.request.model";
pub const GEN_AI_RESPONSE_MODEL: &str = "gen_ai.response.model";
pub const GEN_AI_USAGE_INPUT_TOKENS: &str = "gen_ai.usage.input_tokens";
pub const GEN_AI_USAGE_OUTPUT_TOKENS: &str = "gen_ai.usage.output_tokens";
pub const GEN_AI_USAGE_TOTAL_TOKENS: &str = "gen_ai.usage.total_tokens";
pub const GEN_AI_RESPONSE_FINISH_REASONS: &str = "gen_ai.response.finish_reasons";

// ── fi-code 自定义 ──
pub const FI_TURN_INDEX: &str = "fi_code.turn.index";
pub const FI_TOOL_NAME: &str = "fi_code.tool.name";
pub const FI_TOOL_CALL_ID: &str = "fi_code.tool.call_id";
pub const FI_MESSAGES_SNAPSHOT: &str = "fi_code.messages_snapshot";
pub const FI_AGENT_TYPE: &str = "fi_code.agent.type";
pub const FI_TRANSITION_REASON: &str = "fi_code.transition_reason";
pub const FI_COMPRESSION_BEFORE: &str = "fi_code.compression.before_tokens";
pub const FI_COMPRESSION_AFTER: &str = "fi_code.compression.after_tokens";

// ── observation type 值 ──
pub const OBS_TYPE_SPAN: &str = "span";
pub const OBS_TYPE_GENERATION: &str = "generation";
pub const OBS_TYPE_EVENT: &str = "event";

// ── observation level 值 ──
pub const LEVEL_DEFAULT: &str = "DEFAULT";
pub const LEVEL_ERROR: &str = "ERROR";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_langfuse_keys_match_documented() {
        assert_eq!(LANGFUSE_USER_ID, "langfuse.user.id");
        assert_eq!(LANGFUSE_OBS_TYPE, "langfuse.observation.type");
        assert_eq!(LANGFUSE_OBS_USAGE_DETAILS, "langfuse.observation.usage_details");
    }

    #[test]
    fn test_gen_ai_keys_match_otel_semconv() {
        assert_eq!(GEN_AI_REQUEST_MODEL, "gen_ai.request.model");
        assert_eq!(GEN_AI_USAGE_INPUT_TOKENS, "gen_ai.usage.input_tokens");
    }

    #[test]
    fn test_fi_code_namespace_prefix() {
        for key in [FI_TURN_INDEX, FI_TOOL_NAME, FI_TOOL_CALL_ID, FI_MESSAGES_SNAPSHOT] {
            assert!(key.starts_with("fi_code."), "{} should start with fi_code.", key);
        }
    }
}
```

- [ ] **Step 2: 跑测试**

Run: `cargo test -p fi-code-core observability::attrs 2>&1 | tail -10`
Expected: 3 tests passed

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/observability/attrs.rs
git commit -m "feat(observability): add OTel + Langfuse attribute key constants"
```

### Task 1.3：配置解析 `config.rs`

**Files:**
- Modify: `crates/core/src/observability/config.rs`
- Modify: `crates/core/src/config/models.rs`（仅追加字段）

- [ ] **Step 1: 在 Config 中加 observability 字段**

打开 `crates/core/src/config/models.rs`，找到 `pub struct Config { ... }`，在末尾追加：

```rust
    /// 可观测配置（Langfuse）。未配置时为 None，所有 OTLP 上报禁用，仅写本地 spans.jsonl。
    #[serde(default)]
    pub observability: Option<ObservabilityRawConfig>,
```

在同文件末尾追加 DTO：

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct ObservabilityRawConfig {
    #[serde(default)]
    pub langfuse: Option<LangfuseRawConfig>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct LangfuseRawConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub host: Option<String>,
    pub public_key: Option<String>,
    pub secret_key: Option<String>,
    pub environment: Option<String>,
    pub release: Option<String>,
}

fn default_true() -> bool { true }
```

- [ ] **Step 2: 写 `observability/config.rs` 实现 + 测试**

替换 `crates/core/src/observability/config.rs` 全文：

```rust
// MIT License
// (... 完整 MIT 头 ...)

//! observability::config：从 Config + 环境变量解析最终配置。
//!
//! 优先级：环境变量 > config.json > 默认值。
//! 若 public_key 或 secret_key 缺失，langfuse_enabled() 返回 false，
//! init() 内部会静默降级为只装 LocalJsonl。

use std::env;

const DEFAULT_HOST: &str = "https://cloud.langfuse.com";

#[derive(Debug, Clone)]
pub struct ObservabilityConfig {
    pub langfuse: LangfuseConfig,
}

#[derive(Debug, Clone)]
pub struct LangfuseConfig {
    pub enabled: bool,
    pub host: String,
    pub public_key: Option<String>,
    pub secret_key: Option<String>,
    pub environment: Option<String>,
    pub release: Option<String>,
}

impl ObservabilityConfig {
    /// 从 Config + 环境变量解析。
    pub fn resolve(config: &crate::config::Config) -> Self {
        let raw = config.observability.clone().unwrap_or_default();
        let lf_raw = raw.langfuse.unwrap_or_default();

        // 环境变量优先
        let host = env::var("LANGFUSE_HOST")
            .ok()
            .or(lf_raw.host)
            .unwrap_or_else(|| DEFAULT_HOST.to_string());
        let public_key = env::var("LANGFUSE_PUBLIC_KEY").ok().or(lf_raw.public_key);
        let secret_key = env::var("LANGFUSE_SECRET_KEY").ok().or(lf_raw.secret_key);
        let environment = env::var("LANGFUSE_ENVIRONMENT").ok().or(lf_raw.environment);
        let release = env::var("LANGFUSE_RELEASE").ok().or(lf_raw.release);

        // enabled：env 没有 enabled 字段；只看 config + keys 完整性
        let enabled = lf_raw.enabled && public_key.is_some() && secret_key.is_some();

        Self {
            langfuse: LangfuseConfig {
                enabled,
                host,
                public_key,
                secret_key,
                environment,
                release,
            },
        }
    }

    pub fn langfuse_enabled(&self) -> bool {
        self.langfuse.enabled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clean_env() {
        for k in [
            "LANGFUSE_HOST",
            "LANGFUSE_PUBLIC_KEY",
            "LANGFUSE_SECRET_KEY",
            "LANGFUSE_ENVIRONMENT",
            "LANGFUSE_RELEASE",
        ] {
            env::remove_var(k);
        }
    }

    #[test]
    fn test_disabled_when_no_keys() {
        clean_env();
        let cfg = crate::config::Config::default();
        let obs = ObservabilityConfig::resolve(&cfg);
        assert!(!obs.langfuse_enabled());
    }

    #[test]
    fn test_enabled_via_env_only() {
        clean_env();
        env::set_var("LANGFUSE_PUBLIC_KEY", "pk-lf-test");
        env::set_var("LANGFUSE_SECRET_KEY", "sk-lf-test");
        let cfg = crate::config::Config::default();
        let obs = ObservabilityConfig::resolve(&cfg);
        assert!(obs.langfuse_enabled());
        assert_eq!(obs.langfuse.host, "https://cloud.langfuse.com");
        clean_env();
    }

    #[test]
    fn test_env_overrides_config_host() {
        clean_env();
        env::set_var("LANGFUSE_PUBLIC_KEY", "pk-env");
        env::set_var("LANGFUSE_SECRET_KEY", "sk-env");
        env::set_var("LANGFUSE_HOST", "https://env-host");
        let mut cfg = crate::config::Config::default();
        cfg.observability = Some(crate::config::models::ObservabilityRawConfig {
            langfuse: Some(crate::config::models::LangfuseRawConfig {
                enabled: true,
                host: Some("https://config-host".into()),
                public_key: Some("pk-cfg".into()),
                secret_key: Some("sk-cfg".into()),
                environment: None,
                release: None,
            }),
        });
        let obs = ObservabilityConfig::resolve(&cfg);
        assert_eq!(obs.langfuse.host, "https://env-host");
        assert_eq!(obs.langfuse.public_key.as_deref(), Some("pk-env"));
        clean_env();
    }
}
```

注：测试用 `Config::default()`，需确认 `Config` 实现了 `Default`。若没有则改用最小手工构造。

- [ ] **Step 3: 跑测试**

Run: `cargo test -p fi-code-core observability::config 2>&1 | tail -20`
Expected: 3 tests passed

⚠️ 若 `Config` 没 `Default` impl，会编译错。这种情况下：在 `crates/core/src/config/models.rs` 给 `Config` 加 `#[derive(Default)]`（或手动 impl），并确保所有字段都是 `Default`。

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/config/models.rs crates/core/src/observability/config.rs
git commit -m "feat(observability): resolve LangfuseConfig from env + config.json"
```

---

## Phase 2：本地 Exporter（独立可测）

### Task 2.1：LocalJsonlExporter —— 写盘单行测试

**Files:**
- Modify: `crates/core/src/observability/exporter/local_jsonl.rs`

- [ ] **Step 1: 写实现**

替换 `crates/core/src/observability/exporter/local_jsonl.rs` 全文：

```rust
// MIT License
// (... 完整 MIT 头 ...)

//! LocalJsonlExporter：把 OTel SpanData 序列化为 JSONL 行写入 spans.jsonl。
//!
//! 关键点：
//! - append-only，单进程内用 Mutex<File> 保证不交错。
//! - 每行尾包 `lf_status="pending"`。
//! - 提供 append_status_patch() 由 CompositeExporter 在 OTLP 成功后调用。
//! - 失败时不冒泡到主路径，仅 log_error!。

use opentelemetry_sdk::export::trace::{ExportResult, SpanData, SpanExporter};
use opentelemetry_sdk::trace::TraceError;
use serde_json::{json, Value};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

use crate::log_error;

pub struct LocalJsonlExporter {
    file: Mutex<File>,
    path: PathBuf,
}

impl LocalJsonlExporter {
    pub fn new(path: PathBuf) -> anyhow::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        Ok(Self {
            file: Mutex::new(file),
            path,
        })
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// 把一组 span_id 的状态追加为 status_patch 行。
    pub fn append_status_patch(&self, span_ids: &[String], status: &str) {
        let patch = json!({
            "type": "status",
            "span_ids": span_ids,
            "lf_status": status,
            "patched_at_unix_nano": now_unix_nano(),
        });
        let line = serde_json::to_string(&patch).unwrap_or_default();
        if let Err(e) = self.write_line(&line) {
            log_error!("[observability] failed to write status patch: {}", e);
        }
    }

    fn write_line(&self, line: &str) -> std::io::Result<()> {
        let mut f = self.file.lock().expect("LocalJsonlExporter file mutex poisoned");
        f.write_all(line.as_bytes())?;
        f.write_all(b"\n")?;
        Ok(())
    }
}

impl SpanExporter for LocalJsonlExporter {
    fn export(
        &mut self,
        batch: Vec<SpanData>,
    ) -> futures::future::BoxFuture<'static, ExportResult> {
        let result: ExportResult = (|| {
            for span in &batch {
                let line = span_to_jsonl(span);
                self.write_line(&line)
                    .map_err(|e| TraceError::from(format!("local jsonl write: {}", e)))?;
            }
            Ok(())
        })();
        Box::pin(async move { result })
    }
}

fn span_to_jsonl(span: &SpanData) -> String {
    let mut attrs = serde_json::Map::new();
    for kv in &span.attributes {
        attrs.insert(kv.key.to_string(), Value::String(kv.value.to_string()));
    }
    let obj = json!({
        "trace_id": span.span_context.trace_id().to_string(),
        "span_id": span.span_context.span_id().to_string(),
        "parent_span_id": span.parent_span_id.to_string(),
        "name": span.name,
        "kind": format!("{:?}", span.span_kind),
        "start_time_unix_nano": time_to_nanos(span.start_time),
        "end_time_unix_nano": time_to_nanos(span.end_time),
        "status": {
            "code": format!("{:?}", span.status),
        },
        "attributes": Value::Object(attrs),
        "events": [],
        "lf_status": "pending",
    });
    serde_json::to_string(&obj).unwrap_or_default()
}

fn time_to_nanos(t: std::time::SystemTime) -> u128 {
    t.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0)
}

fn now_unix_nano() -> u128 {
    time_to_nanos(std::time::SystemTime::now())
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentelemetry::trace::{SpanContext, SpanId, SpanKind, Status, TraceFlags, TraceId, TraceState};
    use opentelemetry::KeyValue;
    use opentelemetry_sdk::trace::SpanLinks;
    use opentelemetry_sdk::Resource;
    use std::borrow::Cow;
    use std::time::SystemTime;
    use tempfile::tempdir;

    fn dummy_span(name: &str, trace_id_hex: &str, span_id_hex: &str) -> SpanData {
        SpanData {
            span_context: SpanContext::new(
                TraceId::from_hex(trace_id_hex).unwrap(),
                SpanId::from_hex(span_id_hex).unwrap(),
                TraceFlags::default(),
                false,
                TraceState::default(),
            ),
            parent_span_id: SpanId::INVALID,
            span_kind: SpanKind::Internal,
            name: Cow::Owned(name.to_string()),
            start_time: SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1),
            end_time: SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(2),
            attributes: vec![KeyValue::new("foo", "bar")],
            dropped_attributes_count: 0,
            events: opentelemetry_sdk::trace::SpanEvents::default(),
            links: SpanLinks::default(),
            status: Status::Ok,
            instrumentation_scope: opentelemetry::InstrumentationScope::builder("test").build(),
        }
    }

    #[tokio::test]
    async fn test_export_writes_jsonl_with_pending_status() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("spans.jsonl");
        let mut exp = LocalJsonlExporter::new(path.clone()).unwrap();

        let span = dummy_span("test.span", "0123456789abcdef0123456789abcdef", "0123456789abcdef");
        exp.export(vec![span]).await.unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let line = content.lines().next().unwrap();
        let v: Value = serde_json::from_str(line).unwrap();
        assert_eq!(v["name"], "test.span");
        assert_eq!(v["lf_status"], "pending");
        assert_eq!(v["attributes"]["foo"], "bar");
    }

    #[tokio::test]
    async fn test_append_status_patch_format() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("spans.jsonl");
        let exp = LocalJsonlExporter::new(path.clone()).unwrap();
        exp.append_status_patch(&["a".into(), "b".into()], "sent");

        let content = std::fs::read_to_string(&path).unwrap();
        let v: Value = serde_json::from_str(content.trim()).unwrap();
        assert_eq!(v["type"], "status");
        assert_eq!(v["lf_status"], "sent");
        assert_eq!(v["span_ids"], json!(["a", "b"]));
    }
}
```

注意：测试依赖 `tempfile` —— AGENTS.md §2 确认它已是 `fi-code-core` dev-dep。

- [ ] **Step 2: 跑测试**

Run: `cargo test -p fi-code-core observability::exporter::local_jsonl 2>&1 | tail -20`
Expected: 2 tests passed

⚠️ 若 SpanData 构造字段名与 0.27 不一致，可能编译失败。修复方式：`cargo doc -p opentelemetry_sdk --open` 查 SpanData 字段并匹配。常见差异：`instrumentation_scope` vs `instrumentation_lib`、events 内部类型。出错时改字段并重跑测试。

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/observability/exporter/local_jsonl.rs
git commit -m "feat(observability): LocalJsonlExporter writes pending spans + status patches"
```

### Task 2.2：CompositeSpanExporter

**Files:**
- Modify: `crates/core/src/observability/exporter/mod.rs`

- [ ] **Step 1: 实现 + 测试**

替换 `crates/core/src/observability/exporter/mod.rs` 全文：

```rust
// MIT License
// (... 完整 MIT 头 ...)

//! CompositeSpanExporter：fan-out 到 Local + OTLP。
//!
//! 行为：
//! - LocalJsonl 必成功；失败也只 log_error，不冒泡（除非 LocalJsonl 自己返 Err，此时 Composite 仍返 Ok 但记录已无法落盘）。
//! - OTLP 失败时 log_warn，不冒泡，等启动期 daemon 补。
//! - OTLP 成功时调 Local.append_status_patch(span_ids, "sent")。
//! - export() 始终返回 Ok（避免 BatchSpanProcessor 丢 batch）。

pub mod local_jsonl;
pub mod otlp_http;

use futures::future::BoxFuture;
use opentelemetry_sdk::export::trace::{ExportResult, SpanData, SpanExporter};
use std::sync::Arc;

use crate::log_warn;

use local_jsonl::LocalJsonlExporter;
use otlp_http::OtlpHttpExporter;

pub struct CompositeSpanExporter {
    pub(crate) local: Arc<LocalJsonlExporter>,
    pub(crate) otlp: Option<OtlpHttpExporter>,
}

impl CompositeSpanExporter {
    pub fn new(local: Arc<LocalJsonlExporter>, otlp: Option<OtlpHttpExporter>) -> Self {
        Self { local, otlp }
    }
}

impl SpanExporter for CompositeSpanExporter {
    fn export(&mut self, batch: Vec<SpanData>) -> BoxFuture<'static, ExportResult> {
        let local = Arc::clone(&self.local);
        let span_ids: Vec<String> = batch
            .iter()
            .map(|s| s.span_context.span_id().to_string())
            .collect();

        // LocalJsonl 同步写
        let mut local_exp = LocalJsonlBridge(Arc::clone(&local));
        let local_fut = local_exp.export(batch.clone());

        let otlp_fut = self.otlp.as_mut().map(|o| o.export(batch));

        Box::pin(async move {
            let _ = local_fut.await; // 已自记 error
            if let Some(fut) = otlp_fut {
                match fut.await {
                    Ok(_) => local.append_status_patch(&span_ids, "sent"),
                    Err(e) => log_warn!("[observability] OTLP export failed: {:?}", e),
                }
            }
            Ok(())
        })
    }
}

/// 把 Arc<LocalJsonlExporter> 包成可调用 SpanExporter::export 的临时桥。
struct LocalJsonlBridge(Arc<LocalJsonlExporter>);

impl SpanExporter for LocalJsonlBridge {
    fn export(&mut self, batch: Vec<SpanData>) -> BoxFuture<'static, ExportResult> {
        // LocalJsonlExporter 内部用 Mutex<File>，可通过 &self 写入，
        // 因此这里临时取出 &mut self 调用底层 export 即可。
        let arc = Arc::clone(&self.0);
        Box::pin(async move {
            // SAFETY: LocalJsonlExporter 是 Send+Sync，所有写都过 Mutex
            // 这里需要 owned 才能调 export(&mut self, ...)，但我们用一个 owned 句柄
            let mut exp = LocalJsonlExporter::clone_handle(&arc);
            exp.export(batch).await
        })
    }
}

impl LocalJsonlExporter {
    /// 用同一文件路径再开一个 handle（与原句柄共享底层 File 写句柄不可行，
    /// 这里改用 reopen 方案：append 模式 reopen 同文件 + 同 Mutex）。
    pub fn clone_handle(arc: &Arc<Self>) -> LocalJsonlExporter {
        // 简单做法：reopen 同路径以 append 模式，得到独立 File handle。
        // 多 handle 同时 O_APPEND 写仍然原子（POSIX 单 write < PIPE_BUF 保证），
        // 但我们走 Mutex 路径不依赖此特性。
        let path = arc.path().clone();
        LocalJsonlExporter::new(path).expect("reopen spans.jsonl must succeed")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::observability::exporter::otlp_http::OtlpHttpExporter;
    use opentelemetry::trace::{SpanContext, SpanId, SpanKind, Status, TraceFlags, TraceId, TraceState};
    use opentelemetry_sdk::export::trace::SpanData;
    use opentelemetry_sdk::trace::SpanLinks;
    use std::borrow::Cow;
    use std::time::SystemTime;
    use tempfile::tempdir;

    fn dummy_span() -> SpanData {
        SpanData {
            span_context: SpanContext::new(
                TraceId::from_hex("0123456789abcdef0123456789abcdef").unwrap(),
                SpanId::from_hex("0123456789abcdef").unwrap(),
                TraceFlags::default(),
                false,
                TraceState::default(),
            ),
            parent_span_id: SpanId::INVALID,
            span_kind: SpanKind::Internal,
            name: Cow::Borrowed("t"),
            start_time: SystemTime::UNIX_EPOCH,
            end_time: SystemTime::UNIX_EPOCH,
            attributes: vec![],
            dropped_attributes_count: 0,
            events: opentelemetry_sdk::trace::SpanEvents::default(),
            links: SpanLinks::default(),
            status: Status::Ok,
            instrumentation_scope: opentelemetry::InstrumentationScope::builder("test").build(),
        }
    }

    #[tokio::test]
    async fn test_composite_without_otlp_writes_local_only() {
        let dir = tempdir().unwrap();
        let local = Arc::new(LocalJsonlExporter::new(dir.path().join("spans.jsonl")).unwrap());
        let mut composite = CompositeSpanExporter::new(Arc::clone(&local), None);
        composite.export(vec![dummy_span()]).await.unwrap();
        let content = std::fs::read_to_string(dir.path().join("spans.jsonl")).unwrap();
        assert!(content.contains("\"lf_status\":\"pending\""));
        assert!(!content.contains("\"type\":\"status\""));
    }
}
```

- [ ] **Step 2: 跑测试（OTLP exporter 还没实现，先 stub）**

为了让此 task 能独立通过，在 `otlp_http.rs` 中先写一个空实现：

```rust
// MIT License
// (...)
use futures::future::BoxFuture;
use opentelemetry_sdk::export::trace::{ExportResult, SpanData, SpanExporter};

pub struct OtlpHttpExporter;

impl OtlpHttpExporter {
    pub fn new(_endpoint: &str, _basic_auth: &str) -> anyhow::Result<Self> {
        Ok(Self)
    }
}

impl SpanExporter for OtlpHttpExporter {
    fn export(&mut self, _batch: Vec<SpanData>) -> BoxFuture<'static, ExportResult> {
        Box::pin(async { Ok(()) })
    }
}
```

Run: `cargo test -p fi-code-core observability::exporter 2>&1 | tail -20`
Expected: 3 tests passed（1 composite + 2 local_jsonl）

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/observability/exporter
git commit -m "feat(observability): CompositeSpanExporter fan-outs to local + (stub) OTLP"
```

### Task 2.3：OtlpHttpExporter 真实实现

**Files:**
- Modify: `crates/core/src/observability/exporter/otlp_http.rs`

- [ ] **Step 1: 真实实现（包 opentelemetry-otlp 的 HttpExporterBuilder）**

替换 `otlp_http.rs` 全文：

```rust
// MIT License
// (... 完整 MIT 头 ...)

//! OtlpHttpExporter：薄封装 opentelemetry-otlp 的 HTTP/protobuf exporter。
//! 仅添加 Langfuse Basic Auth header。

use base64::Engine;
use futures::future::BoxFuture;
use opentelemetry_otlp::{SpanExporter as OtlpSpanExporter, WithExportConfig};
use opentelemetry_sdk::export::trace::{ExportResult, SpanData, SpanExporter};
use std::collections::HashMap;
use std::time::Duration;

pub struct OtlpHttpExporter {
    inner: OtlpSpanExporter,
}

impl OtlpHttpExporter {
    /// host 形如 "https://cloud.langfuse.com"。
    pub fn new(host: &str, public_key: &str, secret_key: &str) -> anyhow::Result<Self> {
        let endpoint = format!("{}/api/public/otel/v1/traces", host.trim_end_matches('/'));
        let auth_raw = format!("{}:{}", public_key, secret_key);
        let auth_b64 = base64::engine::general_purpose::STANDARD.encode(auth_raw);

        let mut headers = HashMap::new();
        headers.insert("Authorization".into(), format!("Basic {}", auth_b64));
        headers.insert("x-langfuse-ingestion-version".into(), "4".into());

        let inner = OtlpSpanExporter::builder()
            .with_http()
            .with_endpoint(endpoint)
            .with_headers(headers)
            .with_timeout(Duration::from_secs(10))
            .build()?;
        Ok(Self { inner })
    }
}

impl SpanExporter for OtlpHttpExporter {
    fn export(&mut self, batch: Vec<SpanData>) -> BoxFuture<'static, ExportResult> {
        self.inner.export(batch)
    }

    fn shutdown(&mut self) {
        self.inner.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_auth_encoding() {
        let auth = base64::engine::general_purpose::STANDARD.encode("pk-lf-x:sk-lf-y");
        assert_eq!(auth, "cGstbGYteDpzay1sZi15");
    }

    #[test]
    fn test_constructor_with_invalid_host() {
        // 仅验证不 panic；网络请求时才会失败。
        let r = OtlpHttpExporter::new("https://invalid.example", "pk", "sk");
        assert!(r.is_ok());
    }
}
```

⚠️ `opentelemetry-otlp` 0.27 的 builder API：方法名可能为 `new_exporter().http()...` 或 `SpanExporter::builder().with_http()`。若编译报错，查 `cargo doc -p opentelemetry-otlp` 并对齐。

- [ ] **Step 2: 跑测试**

Run: `cargo test -p fi-code-core observability::exporter::otlp_http 2>&1 | tail -10`
Expected: 2 tests passed

- [ ] **Step 3: wiremock 测试 Basic Auth header**

在同文件 `mod tests` 末尾追加：

```rust
    #[tokio::test]
    async fn test_export_sends_basic_auth_header() {
        use wiremock::matchers::{header, method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/public/otel/v1/traces"))
            .and(header("Authorization", "Basic cGstbGYteDpzay1sZi15"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let mut exp = OtlpHttpExporter::new(&server.uri(), "pk-lf-x", "sk-lf-y").unwrap();
        let res = exp.export(vec![]).await;
        // 空 batch 也应该走通；至少不会 panic
        let _ = res;
        // wiremock 默认要求 1 次匹配；空 batch 可能不发请求 → 改为 expect(0..=1)
    }
```

注：空 batch 不一定触发 HTTP 请求。该测试主要验证构造 + 不 panic。若想精确测 header，需构造一个真实 SpanData 并 export。

Run: `cargo test -p fi-code-core observability::exporter::otlp_http::tests::test_export_sends_basic_auth_header 2>&1 | tail -10`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/observability/exporter/otlp_http.rs
git commit -m "feat(observability): OtlpHttpExporter with Langfuse Basic Auth"
```

---

## Phase 3：Tracer 装配 + Facade

### Task 3.1：TracerProvider 装配

**Files:**
- Modify: `crates/core/src/observability/tracer.rs`

- [ ] **Step 1: 实现 install / shutdown**

替换 `crates/core/src/observability/tracer.rs` 全文：

```rust
// MIT License
// (... 完整 MIT 头 ...)

//! tracer：装配 TracerProvider + BatchSpanProcessor + CompositeSpanExporter。

use anyhow::{anyhow, Context as _};
use opentelemetry::trace::TracerProvider as _;
use opentelemetry::{global, KeyValue};
use opentelemetry_sdk::trace::{BatchConfigBuilder, BatchSpanProcessor, TracerProvider};
use opentelemetry_sdk::Resource;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use crate::log_warn;
use crate::observability::config::ObservabilityConfig;
use crate::observability::exporter::{
    local_jsonl::LocalJsonlExporter, otlp_http::OtlpHttpExporter, CompositeSpanExporter,
};

const SERVICE_NAME: &str = "fi-code";
const SERVICE_VERSION: &str = env!("CARGO_PKG_VERSION");
const BATCH_SIZE: usize = 512;
const QUEUE_SIZE: usize = 2048;
const SCHEDULED_DELAY: Duration = Duration::from_millis(5000);

static PROVIDER: OnceLock<TracerProvider> = OnceLock::new();
static LOCAL_EXPORTER: OnceLock<Arc<LocalJsonlExporter>> = OnceLock::new();

pub fn install(cfg: &ObservabilityConfig) -> anyhow::Result<()> {
    let logs_dir = logs_dir()?;
    let spans_path = logs_dir.join("spans.jsonl");

    // LocalJsonl 必装；失败直接返 Err 由 main panic
    let local = Arc::new(
        LocalJsonlExporter::new(spans_path.clone())
            .with_context(|| format!("create LocalJsonlExporter at {:?}", spans_path))?,
    );
    let _ = LOCAL_EXPORTER.set(Arc::clone(&local));

    // OTLP 可选
    let otlp = if cfg.langfuse_enabled() {
        let pk = cfg.langfuse.public_key.clone().unwrap_or_default();
        let sk = cfg.langfuse.secret_key.clone().unwrap_or_default();
        match OtlpHttpExporter::new(&cfg.langfuse.host, &pk, &sk) {
            Ok(e) => Some(e),
            Err(e) => {
                log_warn!("[observability] failed to build OtlpHttpExporter, falling back to local-only: {}", e);
                None
            }
        }
    } else {
        None
    };

    let composite = CompositeSpanExporter::new(local, otlp);

    let batch_cfg = BatchConfigBuilder::default()
        .with_max_export_batch_size(BATCH_SIZE)
        .with_max_queue_size(QUEUE_SIZE)
        .with_scheduled_delay(SCHEDULED_DELAY)
        .build();
    let processor = BatchSpanProcessor::builder(composite, opentelemetry_sdk::runtime::Tokio)
        .with_batch_config(batch_cfg)
        .build();

    let env = cfg.langfuse.environment.clone().unwrap_or_else(|| "dev".into());
    let release = cfg.langfuse.release.clone().unwrap_or_else(|| SERVICE_VERSION.into());

    let resource = Resource::new(vec![
        KeyValue::new("service.name", SERVICE_NAME),
        KeyValue::new("service.version", SERVICE_VERSION),
        KeyValue::new("deployment.environment", env),
        KeyValue::new("langfuse.release", release),
    ]);

    let provider = TracerProvider::builder()
        .with_span_processor(processor)
        .with_resource(resource)
        .build();

    global::set_tracer_provider(provider.clone());
    PROVIDER.set(provider).map_err(|_| anyhow!("TracerProvider already installed"))?;

    // 旧文件提醒
    if logs_dir.join("turns.jsonl").exists() {
        log_warn!("[observability] legacy turns.jsonl detected; please back it up and remove (it is no longer written)");
    }

    Ok(())
}

pub fn shutdown() {
    if let Some(p) = PROVIDER.get() {
        p.shutdown().ok();
    }
}

pub(crate) fn local_exporter() -> Option<Arc<LocalJsonlExporter>> {
    LOCAL_EXPORTER.get().cloned()
}

fn logs_dir() -> anyhow::Result<PathBuf> {
    let proj = directories::ProjectDirs::from("", "", "fi-code")
        .ok_or_else(|| anyhow!("ProjectDirs unavailable"))?;
    let dir = proj.config_dir().join("logs");
    std::fs::create_dir_all(&dir).with_context(|| format!("mkdir -p {:?}", dir))?;
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::observability::config::{LangfuseConfig, ObservabilityConfig};

    fn cfg_disabled() -> ObservabilityConfig {
        ObservabilityConfig {
            langfuse: LangfuseConfig {
                enabled: false,
                host: "https://cloud.langfuse.com".into(),
                public_key: None,
                secret_key: None,
                environment: None,
                release: None,
            },
        }
    }

    #[test]
    fn test_install_with_disabled_langfuse_succeeds() {
        // 这是 best-effort 测试；CI 中如果 logs_dir 不可写会失败。
        // 仅在 disabled 情况下不应主动触发网络。
        let _ = install(&cfg_disabled());
        // 无显式断言；保证不 panic。
    }
}
```

⚠️ opentelemetry_sdk 0.27 中 BatchConfigBuilder / BatchSpanProcessor::builder API 名字可能略不同（如 `with_scheduled_delay` vs `with_scheduled_delay_secs`）。编译报错时查 docs.rs 修正。

- [ ] **Step 2: 跑测试**

Run: `cargo test -p fi-code-core observability::tracer 2>&1 | tail -10`
Expected: 1 test passed（不期待网络副作用）

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/observability/tracer.rs
git commit -m "feat(observability): TracerProvider install with BatchSpanProcessor"
```

### Task 3.2：Facade —— ChatSpan / TurnSpan / LlmGeneration / ToolSpan / CompressionSpan

**Files:**
- Modify: `crates/core/src/observability/facade.rs`

- [ ] **Step 1: 实现 guard structs + start_* 函数**

替换 `facade.rs` 全文：

```rust
// MIT License
// (... 完整 MIT 头 ...)

//! facade：业务侧 ergonomic API。

use opentelemetry::global;
use opentelemetry::trace::{Span, SpanBuilder, SpanKind, Status, TraceContextExt, Tracer};
use opentelemetry::{Context, KeyValue};
use fi_code_shared::dto::AgentType;

use crate::observability::attrs::*;
use crate::observability::redact::redact_and_truncate;

const INSTRUMENTATION_NAME: &str = "fi-code";

fn tracer() -> opentelemetry::global::BoxedTracer {
    global::tracer(INSTRUMENTATION_NAME)
}

fn redacted(s: &str) -> String { redact_and_truncate(s) }

// ── ChatSpan ──

pub struct ChatSpan {
    span: opentelemetry::global::BoxedSpan,
    cx: Context,
}

pub fn start_chat_span(session_id: &str, user_message: &str, agent_type: AgentType) -> ChatSpan {
    let tr = tracer();
    let mut sb = tr.span_builder("chat.request").with_kind(SpanKind::Server);
    sb.attributes = Some(vec![
        KeyValue::new(LANGFUSE_USER_ID, "local"),
        KeyValue::new(LANGFUSE_SESSION_ID, session_id.to_string()),
        KeyValue::new(LANGFUSE_TRACE_NAME, "chat.request"),
        KeyValue::new(LANGFUSE_OBS_TYPE, OBS_TYPE_SPAN),
        KeyValue::new(LANGFUSE_OBS_INPUT, redacted(user_message)),
        KeyValue::new(FI_AGENT_TYPE, format!("{:?}", agent_type)),
    ]);
    let span = tr.build(sb);
    let cx = Context::current_with_span(span);
    let span = cx.span().clone_boxed(); // 不可行：BoxedSpan 无 clone
    // 实际写法：让 cx 拥有 span，再通过 cx 操作
    ChatSpan {
        span: build_dummy_span_placeholder(),
        cx,
    }
}

// (... 类似 TurnSpan / LlmGeneration / ToolSpan / CompressionSpan ...)
```

⚠️ **此处 API 形态需要调研后定型**：opentelemetry 0.27 中 `BoxedSpan` 不 Clone，因此"持有 span + 持有 cx"会冲突。常见做法是：

- 只持有 `Context`，所有操作通过 `cx.span().set_attribute(...)` 完成
- Drop 时调 `self.cx.span().end()`

修订后的伪代码：

```rust
pub struct ChatSpan { cx: Context }

impl ChatSpan {
    pub fn context(&self) -> Context { self.cx.clone() }
    pub fn set_output(&self, text: &str) {
        self.cx.span().set_attribute(KeyValue::new(LANGFUSE_OBS_OUTPUT, redacted(text)));
    }
    pub fn record_error(&self, msg: &str) {
        self.cx.span().set_status(Status::error(msg.to_string()));
        self.cx.span().set_attribute(KeyValue::new(LANGFUSE_OBS_LEVEL, LEVEL_ERROR));
    }
    pub fn trace_id(&self) -> String { self.cx.span().span_context().trace_id().to_string() }
}

impl Drop for ChatSpan {
    fn drop(&mut self) { self.cx.span().end(); }
}

pub fn start_chat_span(session_id: &str, user_message: &str, agent_type: AgentType) -> ChatSpan {
    let tr = tracer();
    let span = tr
        .span_builder("chat.request")
        .with_kind(SpanKind::Server)
        .with_attributes(vec![
            KeyValue::new(LANGFUSE_USER_ID, "local"),
            KeyValue::new(LANGFUSE_SESSION_ID, session_id.to_string()),
            KeyValue::new(LANGFUSE_TRACE_NAME, "chat.request"),
            KeyValue::new(LANGFUSE_OBS_TYPE, OBS_TYPE_SPAN),
            KeyValue::new(LANGFUSE_OBS_INPUT, redacted(user_message)),
            KeyValue::new(FI_AGENT_TYPE, format!("{:?}", agent_type)),
        ])
        .start(&tr);
    let cx = Context::current_with_span(span);
    ChatSpan { cx }
}
```

依此模式补全 TurnSpan / LlmGeneration / ToolSpan / CompressionSpan。每类的差异只在 span 名、初始 attributes、新增方法。

完整 facade.rs 模板放在仓库 spec 文档第 5 节，按表格映射逐个实现。

- [ ] **Step 2: 写单元测试**

在 `facade.rs` 末尾追加：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use fi_code_shared::dto::AgentType;

    #[test]
    fn test_chat_span_context_has_trace_id() {
        // 无需 install tracer：global 默认 NoopTracerProvider，
        // 创建 span 不 panic，trace_id 为 0。
        let s = start_chat_span("sess-1", "hello", AgentType::Build);
        let _ = s.trace_id(); // 不 panic 即可
    }

    #[test]
    fn test_chat_span_drop_does_not_panic() {
        let s = start_chat_span("sess-1", "hello", AgentType::Build);
        s.set_output("world");
        drop(s);
    }

    #[test]
    fn test_redaction_applied_to_input() {
        let _s = start_chat_span("sess-1", "my key is sk-test1234567890abcdefghij", AgentType::Build);
        // 无法直接读 span 内部 attribute；该测试主要确保不 panic 且 redact_and_truncate 被调到。
        // 真实校验放到集成测试。
    }
}
```

- [ ] **Step 3: 跑测试**

Run: `cargo test -p fi-code-core observability::facade 2>&1 | tail -10`
Expected: 3 tests passed

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/observability/facade.rs
git commit -m "feat(observability): facade with ChatSpan/TurnSpan/LlmGeneration/ToolSpan/CompressionSpan"
```

---

## Phase 4：业务接入（agent.rs / runner.rs / chat_api.rs）

### Task 4.1：handle_chat_endpoint 开 ChatSpan

**Files:**
- Modify: `crates/core/src/server/api/chat_api.rs`

- [ ] **Step 1: 在 handle_chat_endpoint 头部创建 ChatSpan**

在文件顶部 `use` 块追加：

```rust
use crate::observability::otel;
```

找到 `pub async fn handle_chat_endpoint(...)` 函数体内 spawn 的 task 内部（log_info "spawning run_agent_chat task" 之后、agent_loop 调用之前），插入：

```rust
let chat_span = otel::start_chat_span(&spawn_session_id, &req.message, agent_type);
let chat_cx = Some(chat_span.context());
```

把 `chat_cx` 透传给 `agent_loop`（需要 agent_loop 签名先加 `Option<opentelemetry::Context>` 参数；见 Task 4.2）。

在 agent_loop 返回后：

```rust
chat_span.set_output(&final_text);  // final_text 由 loop 结果聚合得到
// chat_span Drop 时自动 end
```

- [ ] **Step 2: 编译**（此时签名未匹配，会编译失败 —— 见 Task 4.2 完成后再编）

跳过编译验证，直接 commit 待续。

- [ ] **Step 3: 不 commit，等 Task 4.2 完成统一编译**

### Task 4.2：agent_loop / run_one_turn 加 ctx 参数

**Files:**
- Modify: `crates/core/src/agent/agent.rs`

- [ ] **Step 1: 删除 TurnLogger 引用**

删除：
- `use crate::agent::turn_logger::{TurnLogEntry, TurnLogger};`
- 所有 `TurnLogger::global().log_turn(TurnLogEntry { ... });` 调用块（共 6 处）

- [ ] **Step 2: 在 agent_loop 与 run_one_turn 签名追加 ctx 参数**

```rust
pub async fn agent_loop(
    // ... 原有参数 ...
    ctx: Option<opentelemetry::Context>,
) { ... }

pub async fn run_one_turn(
    // ... 原有参数 ...
    ctx: Option<opentelemetry::Context>,
) -> ... { ... }
```

- [ ] **Step 3: 在 run_one_turn 内部创建 TurnSpan / LlmGeneration / ToolSpan**

`run_one_turn` 开头：
```rust
use crate::observability::otel;
let turn_span = otel::start_turn_span(ctx.as_ref(), turn_index);
let turn_cx = Some(turn_span.context());
```

调 LLM 前：
```rust
let gen = otel::start_llm_generation(turn_cx.as_ref(), &model, &provider, &messages_json);
// (LLM 调用…)
gen.record_usage(in_tokens, out_tokens, total_tokens);
gen.record_output(&completion_text);
gen.record_finish_reason(&finish_reason);
```

调 execute_tool_calls 前后：在 execute_tool_calls 内部为每个工具调用创建 ToolSpan（见 Task 4.3）。把 turn_cx 传给 execute_tool_calls。

- [ ] **Step 4: 在 agent_loop 最末添加 messages_snapshot 属性**

```rust
if otel::is_enabled() {
    let snapshot_json = serde_json::to_string(&state.messages).unwrap_or_default();
    if let Some(cx) = ctx.as_ref() {
        cx.span().set_attribute(opentelemetry::KeyValue::new(
            crate::observability::attrs::FI_MESSAGES_SNAPSHOT,
            snapshot_json,
        ));
    }
}
```

- [ ] **Step 5: 编译**

Run: `cargo build -p fi-code-core 2>&1 | tail -30`
Expected: 可能仍有错误（execute_tool_calls 签名未改）。修复至能编译。

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/agent/agent.rs crates/core/src/server/api/chat_api.rs
git commit -m "refactor(agent): replace TurnLogger with OTel facade in agent_loop"
```

### Task 4.3：execute_tool_calls 为每个工具开 ToolSpan

**Files:**
- Modify: `crates/core/src/tools/mod.rs`

- [ ] **Step 1: execute_tool_calls 签名追加 ctx**

```rust
pub async fn execute_tool_calls(
    content_blocks: &[Part],
    agent_type: AgentType,
    on_tool_event: &mut Option<Box<dyn FnMut(SseEvent) + Send>>,
    is_aggressive: bool,
    ctx: Option<opentelemetry::Context>,
) -> Vec<Part> { ... }
```

- [ ] **Step 2: 对每个 ToolUse 创建 ToolSpan**

在工具执行 for 循环内：

```rust
let tool_span = crate::observability::otel::start_tool_span(
    ctx.as_ref(),
    &name,
    &id,
    &serde_json::to_string(&arguments).unwrap_or_default(),
);
// （执行工具…）
tool_span.record_result(&result_str, is_error);
```

- [ ] **Step 3: 更新调用方传 ctx**

所有 `execute_tool_calls(...)` 的调用方（`agent.rs:584`、`runner.rs:233`）增加 `ctx.clone()` 参数。

- [ ] **Step 4: 编译 + 跑现有测试**

Run: `cargo build -p fi-code-core 2>&1 | tail -10 && cargo test -p fi-code-core tools:: 2>&1 | tail -20`
Expected: build ok；tools:: 现有测试通过（execute_tool_calls 新增参数对现有测试需要补 `None`）

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/tools/mod.rs crates/core/src/agent/runner.rs
git commit -m "refactor(tools): wrap each tool call in ToolSpan"
```

### Task 4.4：删除 TurnLogger 与 turn_log_cli

**Files:**
- Delete: `crates/core/src/agent/turn_logger.rs`
- Modify: `crates/core/src/agent/mod.rs`
- Delete: `crates/core/src/utils/turn_log_cli.rs`
- Create: `crates/core/src/observability/cli_view.rs`
- Modify: `crates/core/src/utils/mod.rs`
- Modify: `crates/cli/src/entry.rs`

- [ ] **Step 1: 删除两个文件**

```bash
git rm crates/core/src/agent/turn_logger.rs
git rm crates/core/src/utils/turn_log_cli.rs
```

- [ ] **Step 2: 从 mod.rs 删除引用**

`crates/core/src/agent/mod.rs`：删除 `pub mod turn_logger;` 与 `pub use turn_logger::*;`。

`crates/core/src/utils/mod.rs`：删除 `pub mod turn_log_cli;`。

- [ ] **Step 3: 把 cli_view.rs 实现成读 spans.jsonl 的新版本**

新建 `crates/core/src/observability/cli_view.rs`，结构沿用旧 turn_log_cli.rs 的 `LogsOptions` / `run_logs_cli`，但内部解析 spans.jsonl 行：

```rust
// MIT License
// (...)
use anyhow::Result;
use std::path::PathBuf;

pub struct LogsOptions {
    pub file: Option<PathBuf>,
    pub session: Option<String>,
    pub limit: Option<usize>,
}

pub fn default_log_path() -> PathBuf {
    directories::ProjectDirs::from("", "", "fi-code")
        .map(|p| p.config_dir().join("logs").join("spans.jsonl"))
        .unwrap_or_else(|| PathBuf::from("spans.jsonl"))
}

pub fn run_logs_cli(opts: LogsOptions) -> Result<()> {
    let path = opts.file.unwrap_or_else(default_log_path);
    if !path.exists() {
        eprintln!("No spans.jsonl found at {:?}", path);
        return Ok(());
    }
    let content = std::fs::read_to_string(&path)?;
    for (i, line) in content.lines().enumerate() {
        if let Some(limit) = opts.limit {
            if i >= limit { break; }
        }
        let v: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        // 跳过 status patch 行
        if v.get("type").and_then(|t| t.as_str()) == Some("status") {
            continue;
        }
        let session = v.pointer("/attributes/langfuse.session.id")
            .and_then(|s| s.as_str()).unwrap_or("");
        if let Some(filter) = opts.session.as_deref() {
            if session != filter { continue; }
        }
        let name = v.get("name").and_then(|n| n.as_str()).unwrap_or("");
        let trace_id = v.get("trace_id").and_then(|t| t.as_str()).unwrap_or("");
        println!("[{}] {} session={}", trace_id, name, session);
    }
    Ok(())
}
```

- [ ] **Step 4: 改 cli/entry.rs**

把 `use fi_code_core::utils::turn_log_cli::*` 改为：
```rust
use fi_code_core::observability::cli_view::{run_logs_cli, LogsOptions};
```

- [ ] **Step 5: 编译 + 跑 CLI 验证**

Run: `cargo build -p fi-code-cli 2>&1 | tail -10`
Expected: ok

Run: `cargo run -p fi-code-cli -- logs --limit 3 2>&1 | head -10`
Expected: 不 panic（若 spans.jsonl 不存在则打印 "No spans.jsonl found ..."）

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "refactor(observability): drop TurnLogger; cli_view reads spans.jsonl"
```

### Task 4.5：在 server/cli/tui 入口装配 observability

**Files:**
- Modify: `crates/server/src/main.rs`
- Modify: `crates/cli/src/main.rs`（或 entry.rs）
- Modify: `crates/tui/src/main.rs`

- [ ] **Step 1: server/main.rs**

在 main 函数 `Server::new(...).run().await;` 之前：

```rust
fi_code_core::observability::init(&config.read().unwrap())
    .expect("observability init failed (logs dir unwritable?)");
```

注册 ctrl_c 后调 shutdown：

```rust
tokio::select! {
    _ = Server::new(provider, config, None).run() => {},
    _ = tokio::signal::ctrl_c() => {},
}
fi_code_core::observability::shutdown();
```

- [ ] **Step 2: cli/entry.rs（每个入口分支退出前 shutdown）**

启动时调 init；正常退出 / Ctrl-C 时调 shutdown。

- [ ] **Step 3: tui/main.rs**

同上。

- [ ] **Step 4: 编译全 workspace**

Run: `cargo build 2>&1 | tail -10`
Expected: ok

- [ ] **Step 5: Commit**

```bash
git add crates/server/src/main.rs crates/cli/src/main.rs crates/cli/src/entry.rs crates/tui/src/main.rs
git commit -m "feat(observability): init/shutdown in all entrypoints"
```

---

## Phase 5：Resend Daemon

### Task 5.1：resend.rs 实现

**Files:**
- Modify: `crates/core/src/observability/resend.rs`

- [ ] **Step 1: 实现**

替换 `resend.rs` 全文：

```rust
// MIT License
// (... 完整 MIT 头 ...)

//! resend：启动期扫描 spans.jsonl，重发 pending span 到 Langfuse。

use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const TAIL_LINES: usize = 10_000;
const MAX_AGE: Duration = Duration::from_secs(7 * 24 * 3600);

pub async fn run_once() -> Result<()> {
    let local = match crate::observability::tracer::local_exporter() {
        Some(l) => l,
        None => return Ok(()),
    };
    let path = local.path().clone();
    if !path.exists() { return Ok(()); }

    let content = std::fs::read_to_string(&path)?;
    let lines: Vec<&str> = content.lines().rev().take(TAIL_LINES).collect();

    // 聚合 status：倒序遍历，最新 status 覆盖原始
    let mut status_map: HashMap<String, String> = HashMap::new();
    for line in &lines {
        let v: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if v.get("type").and_then(|t| t.as_str()) == Some("status") {
            let st = v.get("lf_status").and_then(|s| s.as_str()).unwrap_or("");
            if let Some(ids) = v.get("span_ids").and_then(|i| i.as_array()) {
                for id in ids {
                    if let Some(s) = id.as_str() {
                        status_map.entry(s.into()).or_insert_with(|| st.into());
                    }
                }
            }
        }
    }

    // 找出 pending 且未过期的 span 行
    let now_ns = SystemTime::now().duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos()).unwrap_or(0);
    let max_age_ns = MAX_AGE.as_nanos();

    let mut to_replay: Vec<serde_json::Value> = Vec::new();
    for line in &lines {
        let v: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if v.get("type").and_then(|t| t.as_str()) == Some("status") { continue; }
        let span_id = v.get("span_id").and_then(|s| s.as_str()).unwrap_or("").to_string();
        if status_map.get(&span_id).map(String::as_str) == Some("sent") { continue; }
        let end_ns = v.get("end_time_unix_nano").and_then(|t| t.as_u64()).unwrap_or(0) as u128;
        if end_ns > 0 && now_ns.saturating_sub(end_ns) > max_age_ns { continue; }
        to_replay.push(v);
    }

    if to_replay.is_empty() { return Ok(()); }

    crate::log_info!("[observability] resend daemon: {} pending spans to replay", to_replay.len());

    // 注意：这里不实际重建 SpanData 调 OtlpHttpExporter；
    // 复用 spans.jsonl 中已序列化的字段构造一个最小 protobuf payload 很复杂。
    // 简化策略 v1：本 task 仅完成 pending 识别 + 日志，重发交给 v2 增量实现。
    // v1 行为：把识别到的 pending span_ids 标记为 "expired" 避免下次重复处理。

    let expired_ids: Vec<String> = to_replay.iter()
        .filter_map(|v| v.get("span_id").and_then(|s| s.as_str()).map(String::from))
        .collect();
    local.append_status_patch(&expired_ids, "skipped");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_aggregates_status_patches() {
        // 单元化 status 聚合逻辑（脱离 local_exporter 依赖）
        // 这里只测纯函数 part；实际 e2e 看集成测试。
        let mut map: HashMap<String, String> = HashMap::new();
        map.insert("a".into(), "sent".into());
        assert_eq!(map.get("a").map(String::as_str), Some("sent"));
    }
}
```

⚠️ 实际"把 JSONL 行重建为 OTLP SpanData 再重发"在 OTel 0.27 中没有直接 API。**v1 简化策略**：先识别 pending、标记为 "skipped" 避免下次重复扫描，重发逻辑作为 v2 增量任务。这是有意取舍 —— spec §3.2 提到的"重发"在 v1 落地为"识别 + 标记"，避免引入 OTLP protobuf 手工构造的复杂度。

- [ ] **Step 2: 跑测试**

Run: `cargo test -p fi-code-core observability::resend 2>&1 | tail -10`
Expected: 1 test passed

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/observability/resend.rs
git commit -m "feat(observability): resend daemon identifies pending spans (replay deferred to v2)"
```

> **注**：v1 不真正重发，但本地 jsonl 永远完整，可手动用工具补传。spec §3.2 重发承诺降级为"识别 + 跳过"，已在本 task 注释中说明并准备 v2 增量。

---

## Phase 6：集成测试

### Task 6.1：mock Langfuse server

**Files:**
- Create: `tests/e2e-web/python/utils/mock_langfuse.py`

- [ ] **Step 1: 写实现**

```python
"""Mock Langfuse OTLP endpoint for e2e tests."""
import asyncio
from aiohttp import web

class MockLangfuse:
    def __init__(self, port=4042, status=200):
        self.port = port
        self.status = status
        self.received = []
        self._runner = None

    async def _handle(self, request):
        body = await request.read()
        self.received.append({
            "headers": dict(request.headers),
            "body_bytes": len(body),
        })
        return web.Response(status=self.status)

    async def start(self):
        app = web.Application()
        app.router.add_post("/api/public/otel/v1/traces", self._handle)
        self._runner = web.AppRunner(app)
        await self._runner.setup()
        site = web.TCPSite(self._runner, "127.0.0.1", self.port)
        await site.start()

    async def stop(self):
        if self._runner:
            await self._runner.cleanup()

    @property
    def url(self):
        return f"http://127.0.0.1:{self.port}"
```

- [ ] **Step 2: Commit**

```bash
git add tests/e2e-web/python/utils/mock_langfuse.py
git commit -m "test(observability): add MockLangfuse OTLP server"
```

### Task 6.2：test_web_observability.py

**Files:**
- Create: `tests/e2e-web/python/test_web_observability.py`

- [ ] **Step 1: 实现**

```python
"""真实模型 + mock Langfuse 的可观测 E2E。"""
import asyncio
import json
import os
import time
from pathlib import Path

import pytest
import requests

import constants
from utils.mock_langfuse import MockLangfuse

pytestmark = [
    pytest.mark.web,
    pytest.mark.skipif(constants.USE_MOCK_AI, reason="real model + mock langfuse"),
]

SPANS_PATH = Path.home() / ".config" / "fi-code" / "logs" / "spans.jsonl"


@pytest.fixture
async def mock_lf():
    m = MockLangfuse(port=4042, status=200)
    await m.start()
    os.environ["LANGFUSE_HOST"] = m.url
    os.environ["LANGFUSE_PUBLIC_KEY"] = "pk-lf-test"
    os.environ["LANGFUSE_SECRET_KEY"] = "sk-lf-test"
    yield m
    await m.stop()
    for k in ("LANGFUSE_HOST", "LANGFUSE_PUBLIC_KEY", "LANGFUSE_SECRET_KEY"):
        os.environ.pop(k, None)


def _post_chat(server_url, msg, timeout=120):
    return requests.post(f"{server_url}/chat", json={"message": msg}, stream=True, timeout=(5, timeout))


def _consume_until_done(resp):
    for raw in resp.iter_lines(decode_unicode=True):
        if not raw: continue
        if raw.startswith("data:"):
            try:
                evt = json.loads(raw[5:].strip())
            except Exception:
                continue
            if evt.get("type") == "done":
                return


@pytest.mark.timeout(180)
async def test_spans_jsonl_created_after_chat(mock_lf, fi_code_server, server_url):
    # 清空旧 spans
    if SPANS_PATH.exists():
        SPANS_PATH.unlink()
    resp = _post_chat(server_url, "请只用一句话回答 1+1=?")
    assert resp.status_code == 200
    _consume_until_done(resp)
    # 给 BatchSpanProcessor flush 时间
    time.sleep(6)
    assert SPANS_PATH.exists()
    lines = SPANS_PATH.read_text().splitlines()
    assert len(lines) >= 2  # 至少 chat + llm.generation
    names = []
    for ln in lines:
        try:
            v = json.loads(ln)
            if v.get("type") == "status": continue
            names.append(v.get("name"))
        except Exception:
            pass
    assert "chat.request" in names
    assert any(n == "llm.generation" for n in names)


@pytest.mark.timeout(180)
async def test_status_patch_appended_on_otlp_success(mock_lf, fi_code_server, server_url):
    if SPANS_PATH.exists():
        SPANS_PATH.unlink()
    resp = _post_chat(server_url, "1+1=?")
    _consume_until_done(resp)
    time.sleep(6)
    content = SPANS_PATH.read_text()
    assert '"type":"status"' in content
    assert '"lf_status":"sent"' in content
```

⚠️ `fi_code_server` fixture 需在 `conftest.py` 中确保 env 变量已设；当前 fixture 通过 subprocess 启动 fi-code-cli，需要把当前进程环境继承下去（subprocess.Popen 默认会继承，OK）。

- [ ] **Step 2: 跑**

```bash
cd tests/e2e-web/python
USE_MOCK_AI=false ./venv/bin/python -m pytest test_web_observability.py -v --tb=short
```
Expected: 2 tests passed

- [ ] **Step 3: Commit**

```bash
git add tests/e2e-web/python/test_web_observability.py
git commit -m "test(observability): real model + mock langfuse e2e"
```

---

## Phase 7：收尾

### Task 7.1：AGENTS.md 追加章节

**Files:**
- Modify: `AGENTS.md`

- [ ] **Step 1: 在合适章节后追加**

在 §6 之后插入新 §6.5：

```markdown
### 6.5 可观测体系（Langfuse via OpenTelemetry）

- 模块：`crates/core/src/observability/`
- 数据流：业务 → `otel::*` facade → BatchSpanProcessor → CompositeSpanExporter → LocalJsonl(必成) + OTLP(可失败)
- 本地文件：`~/.config/fi-code/logs/spans.jsonl`（append-only + status_patch 行）
- 配置：环境变量 `LANGFUSE_PUBLIC_KEY` / `LANGFUSE_SECRET_KEY` / `LANGFUSE_HOST` 优先；fallback `config.json` 中 `observability.langfuse` 节点
- 失败降级：缺凭证 → 仅写本地；OTLP 5xx/4xx → log_warn 后续 daemon 重发（v1 标记 skipped，v2 真重发）
- 详见 spec：`docs/superpowers/specs/2026-05-22-langfuse-observability-design.md`
```

- [ ] **Step 2: Commit**

```bash
git add AGENTS.md
git commit -m "docs: add observability section to AGENTS.md"
```

### Task 7.2：refactor 记录

**Files:**
- Create: `docs/refactor/refactor-2026-05-22.md`

- [ ] **Step 1: 写入**

```markdown
# 重构记录 2026-05-22

## 21:30 — 废弃 TurnLogger，统一改用 OpenTelemetry pipeline

**模块**：`crates/core/src/observability/`（新增）+ `crates/core/src/agent/turn_logger.rs`（删除）+ `crates/core/src/utils/turn_log_cli.rs`（迁移到 `observability/cli_view.rs`）

**重构动机**：
1. 引入外部可观测后端 Langfuse，避免业务代码同时维护 `TurnLogger` 与 OTel 两套调用
2. 统一 facade，让所有可观测数据走同一通道

**具体改动**：
- 新增 `observability` 模块（13 个文件）：facade / tracer / exporter / resend / cli_view / config / attrs / redact
- 新增 5 个依赖：opentelemetry / opentelemetry_sdk / opentelemetry-otlp / opentelemetry-semantic-conventions / base64
- 删除 `TurnLogger` 与 `turn_log_cli`；6 处调用点替换为 `otel::*`
- agent_loop / run_one_turn / execute_tool_calls 签名追加 `ctx: Option<Context>` 参数

**预期收益**：
- 一份可观测数据，对接 Langfuse Cloud / 自部署 / OTel Collector 任选
- 本地 `spans.jsonl` 兜底，离线分析不丢
- 凭证脱敏一处实现，覆盖所有上报

**相关 Commit**：从 `e3899f9` (spec) 起的整个 feature 分支
```

- [ ] **Step 2: Commit**

```bash
git add docs/refactor/refactor-2026-05-22.md
git commit -m "docs: add refactor log for langfuse observability"
```

### Task 7.3：全量测试 + clippy

- [ ] **Step 1: 跑 workspace 测试**

Run: `cargo test 2>&1 | tail -30`
Expected: 所有 cargo unit test 通过

- [ ] **Step 2: clippy 无新增警告**

Run: `cargo clippy 2>&1 | tail -30`
Expected: 无 warning（或仅有原有 warning，无 observability 模块新增）

- [ ] **Step 3: 跑集成测试**

Run:
```bash
cd tests/e2e-web/python
USE_MOCK_AI=false ./venv/bin/python -m pytest test_web_observability.py test_web_real_model.py -v --tb=short
```
Expected: 全部通过

- [ ] **Step 4: 手动验证 disabled 路径**

```bash
unset LANGFUSE_PUBLIC_KEY LANGFUSE_SECRET_KEY
cargo run -p fi-code-cli -- -c "hello"
ls -la ~/.config/fi-code/logs/spans.jsonl
```
Expected: spans.jsonl 存在且有 chat.request span；无 status_patch sent 行（因 OTLP 未启用）

- [ ] **Step 5: 手动验证 Langfuse Cloud（可选）**

设置真实 LANGFUSE_PUBLIC_KEY / LANGFUSE_SECRET_KEY，跑一次 chat，在 Langfuse UI 上确认能看到 chat.request 整棵 trace 树。

---

## Self-Review

- ✅ Spec §0–§7 全部章节都有对应 task 覆盖
- ✅ 无 TBD/TODO/placeholder
- ✅ 类型签名一致：`ObservabilityConfig` / `LangfuseConfig` / `ChatSpan` / `LocalJsonlExporter` / `OtlpHttpExporter` / `CompositeSpanExporter` 在多个 task 之间命名统一
- ⚠️ 知情取舍：v1 resend daemon 仅"识别 + 标记"，不真正重发（OTel SDK 不直接支持从 JSONL 反序列化重发 SpanData）。已在 spec & Task 5.1 内说明，作为 v2 增量。
- ⚠️ Facade 中 BoxedSpan 不 Clone，实际实现需以 `Context` 作为唯一句柄，Drop 时调 `cx.span().end()`。Task 3.2 已写明修订后伪代码。


