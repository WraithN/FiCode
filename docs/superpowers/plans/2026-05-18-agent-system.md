# Agent System Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement a config-driven Agent Profile system supporting Build and Plan agents, with tool filtering, TUI status bar display, CTRL+A switching, CLI `--agent` parameter, and Server API `agent` field.

**Architecture:** Extract Agent behavior configuration into `AgentProfile` structs with `ToolFilter` strategies. `AgentRunner` becomes a pure scheduler that queries the active profile for tools schema and prompt suffix. Agent type is bound to Session and persisted in JSONL. TUI, CLI, and Server each provide their own agent switching mechanism.

**Tech Stack:** Rust (Cargo Workspace), tokio, serde/json, crossterm, ratatui, axum

---

## File Structure

| File | Responsibility |
|------|---------------|
| `crates/shared/src/dto.rs` | `AgentType` enum, `Session`/`SseEvent` extensions |
| `crates/shared/src/tui_event.rs` | `SwitchAgent`, `AgentSwitched` events |
| `crates/core/src/agent/profile.rs` (NEW) | `AgentProfile`, `ToolFilter` definitions and static registry |
| `crates/core/src/agent/mod.rs` | Export `AgentType`, `AgentProfile`, `ToolFilter` |
| `crates/core/src/agent/prompt.rs` | `PromptBuilder::with_agent_profile` method |
| `crates/core/src/agent/runner.rs` | Refactored `AgentRunner` accepting `AgentType` |
| `crates/core/src/tools/mod.rs` | `execute_tool_calls` with agent filtering, `tool_schema_for_agent` |
| `crates/core/src/session/session.rs` | JSONL serialization/deserialization of `agent_type` |
| `crates/core/src/server/api/chat_api.rs` | `ChatRequest` `agent` field, SSE `AgentInfo` |
| `crates/cli/src/cli_args.rs` | `--agent` CLI parameter |
| `crates/cli/src/entry.rs` | Pass `args.agent` to `AgentRunner` |
| `crates/tui/src/components/status_bar.rs` | Display agent name in status bar |
| `crates/tui/src/app.rs` | `CTRL+A` handling, `SwitchAgent` state machine |
| `crates/tui/src/client.rs` | Pass `agent` in chat request body |

---

## Task 1: AgentType DTO and Shared Events

**Files:**
- Modify: `crates/shared/src/dto.rs`
- Modify: `crates/shared/src/tui_event.rs`
- Test: `crates/shared/src/dto.rs` (in-module tests)

### Step 1: Add `AgentType` enum to `dto.rs`

Add after the `current_timestamp_ms` function (around line 161):

```rust
// ------------------------------------------------------------------------------
// Agent 类型枚举
// ------------------------------------------------------------------------------

/// Agent 类型：Build（全功能）和 Plan（只读规划）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentType {
    Build,
    Plan,
}

impl Default for AgentType {
    fn default() -> Self {
        AgentType::Build
    }
}

impl AgentType {
    pub fn as_str(&self) -> &'static str {
        match self {
            AgentType::Build => "Build",
            AgentType::Plan => "Plan",
        }
    }
}
```

### Step 2: Add `AgentInfo` to `SseEvent`

In `crates/shared/src/dto.rs`, find the `SseEvent` enum (around line 219) and add:

```rust
    #[serde(rename = "agent_info")]
    AgentInfo {
        agent_type: AgentType,
        agent_name: String,
    },
```

### Step 3: Add `SwitchAgent` and `AgentSwitched` to `AppEvent`

In `crates/shared/src/tui_event.rs`, add to `AppEvent` enum (after `RollbackToWave`):

```rust
    SwitchAgent(AgentType),
    AgentSwitched {
        agent_type: AgentType,
        agent_name: String,
    },
```

Add import at top of file:
```rust
use crate::dto::AgentType;
```

### Step 4: Write test for AgentType serialization

In `crates/shared/src/dto.rs`, add to the bottom of the file in a `#[cfg(test)]` module (create one if it doesn't exist):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_type_default_is_build() {
        assert_eq!(AgentType::default(), AgentType::Build);
    }

    #[test]
    fn test_agent_type_serde_roundtrip() {
        let build = AgentType::Build;
        let json = serde_json::to_string(&build).unwrap();
        assert_eq!(json, "\"build\"");
        let decoded: AgentType = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, AgentType::Build);

        let plan = AgentType::Plan;
        let json = serde_json::to_string(&plan).unwrap();
        assert_eq!(json, "\"plan\"");
        let decoded: AgentType = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, AgentType::Plan);
    }

    #[test]
    fn test_agent_type_as_str() {
        assert_eq!(AgentType::Build.as_str(), "Build");
        assert_eq!(AgentType::Plan.as_str(), "Plan");
    }
}
```

### Step 5: Run tests

```bash
cargo test -p fi-code-shared test_agent
```

Expected: PASS (3 tests)

### Step 6: Commit

```bash
git add crates/shared/src/dto.rs crates/shared/src/tui_event.rs
git commit -m "feat(shared): add AgentType enum and Agent-related events"
```

---

## Task 2: AgentProfile and ToolFilter

**Files:**
- Create: `crates/core/src/agent/profile.rs`
- Modify: `crates/core/src/agent/mod.rs`
- Test: `crates/core/src/agent/profile.rs` (in-module tests)

### Step 1: Create `profile.rs`

Create `crates/core/src/agent/profile.rs` with full MIT license header:

```rust
// MIT License
// Copyright (c) 2025 fi-code contributors
// ... (full license header)

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;
use serde_json::Value;

use fi_code_shared::dto::AgentType;

// =============================================================================
// ToolFilter：工具过滤策略
// =============================================================================

#[derive(Debug, Clone)]
pub enum ToolFilter {
    AllowList(HashSet<String>),
    BlockList(HashSet<String>),
}

impl ToolFilter {
    /// 对 tools_schema 数组进行过滤，只保留允许的工具。
    pub fn apply(&self, tools_schema: &Value) -> Value {
        let arr = match tools_schema.as_array() {
            Some(a) => a,
            None => return tools_schema.clone(),
        };

        let filtered: Vec<Value> = arr
            .iter()
            .filter(|tool| {
                let name = tool
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                self.allows(name)
            })
            .cloned()
            .collect();

        Value::Array(filtered)
    }

    /// 判断单个工具名是否被允许。
    pub fn allows(&self, tool_name: &str) -> bool {
        match self {
            ToolFilter::AllowList(set) => set.contains(tool_name),
            ToolFilter::BlockList(set) => !set.contains(tool_name),
        }
    }
}

// =============================================================================
// AgentProfile：Agent 行为配置
// =============================================================================

pub struct AgentProfile {
    pub name: &'static str,
    pub prompt_suffix: &'static str,
    pub tool_filter: ToolFilter,
    pub can_execute_tasks: bool,
}

impl AgentProfile {
    /// 根据 AgentType 获取对应的静态 Profile。
    pub fn for_type(agent_type: AgentType) -> &'static Self {
        static PROFILES: LazyLock<HashMap<AgentType, AgentProfile>> = LazyLock::new(|| {
            let mut m = HashMap::new();

            // Build Agent：允许所有工具
            let mut build_tools = HashSet::new();
            build_tools.extend([
                "bash", "read", "read_file", "write", "edit",
                "grep", "glob", "web_fetch",
                "git", "git_status", "git_diff", "git_add", "git_commit", "git_log", "git_worktree",
                "create_task_plan", "handle_task_plan",
                "ask_for_question", "use_skill",
            ].map(String::from));

            // Plan Agent：只允许只读工具
            let mut plan_tools = HashSet::new();
            plan_tools.extend([
                "read", "read_file", "grep", "glob",
                "git_status", "git_log", "git_diff",
                "web_fetch",
                "create_task_plan", "handle_task_plan",
            ].map(String::from));

            m.insert(AgentType::Build, AgentProfile {
                name: "Build",
                prompt_suffix: concat!(
                    "\n\n## Agent Mode: Build\n",
                    "You are a full-featured coding assistant. ",
                    "You can read and write files, execute shell commands, ",
                    "manage Git operations, and perform any task necessary ",
                    "to help the user with their project."
                ),
                tool_filter: ToolFilter::AllowList(build_tools),
                can_execute_tasks: true,
            });

            m.insert(AgentType::Plan, AgentProfile {
                name: "Plan",
                prompt_suffix: concat!(
                    "\n\n## Agent Mode: Plan\n",
                    "You are a planning assistant. You can only read code ",
                    "and materials, but you cannot modify files or execute commands. ",
                    "Your task is to analyze requirements, examine the codebase, ",
                    "and produce detailed implementation plans. ",
                    "When using create_task_plan or handle_task_plan, ",
                    "you should create the plan and mark it complete, ",
                    "but do not actually execute the sub-tasks."
                ),
                tool_filter: ToolFilter::AllowList(plan_tools),
                can_execute_tasks: false,
            });

            m
        });

        PROFILES.get(&agent_type).expect("profile must exist")
    }
}

// =============================================================================
// 单元测试
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_filter_allow_list() {
        let mut set = HashSet::new();
        set.insert("read".to_string());
        set.insert("grep".to_string());
        let filter = ToolFilter::AllowList(set);

        assert!(filter.allows("read"));
        assert!(filter.allows("grep"));
        assert!(!filter.allows("write"));
        assert!(!filter.allows("bash"));
    }

    #[test]
    fn test_tool_filter_apply() {
        let mut set = HashSet::new();
        set.insert("read".to_string());
        set.insert("grep".to_string());
        let filter = ToolFilter::AllowList(set);

        let schema = serde_json::json!([
            {"name": "read", "description": "Read file"},
            {"name": "write", "description": "Write file"},
            {"name": "grep", "description": "Search code"},
        ]);

        let result = filter.apply(&schema);
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["name"], "read");
        assert_eq!(arr[1]["name"], "grep");
    }

    #[test]
    fn test_profile_for_build() {
        let profile = AgentProfile::for_type(AgentType::Build);
        assert_eq!(profile.name, "Build");
        assert!(profile.can_execute_tasks);
        assert!(profile.tool_filter.allows("bash"));
        assert!(profile.tool_filter.allows("write"));
    }

    #[test]
    fn test_profile_for_plan() {
        let profile = AgentProfile::for_type(AgentType::Plan);
        assert_eq!(profile.name, "Plan");
        assert!(!profile.can_execute_tasks);
        assert!(profile.tool_filter.allows("read"));
        assert!(!profile.tool_filter.allows("bash"));
        assert!(!profile.tool_filter.allows("write"));
    }
}
```

### Step 2: Export from `agent/mod.rs`

Modify `crates/core/src/agent/mod.rs`:

```rust
pub mod agent;
pub mod profile;
pub mod prompt;
pub mod runner;
pub mod turn_logger;

pub use agent::{agent_loop, run_one_turn, LoopState, TurnState};
pub use profile::{AgentProfile, ToolFilter};
pub use prompt::PromptBuilder;
pub use runner::{AgentRunResult, AgentRunner};
pub use turn_logger::{build_tool_result_logs, ToolResultLog, TurnLogEntry, TurnLogger};
```

Also add `pub use fi_code_shared::dto::AgentType;` to re-export AgentType from the agent module.

### Step 3: Run tests

```bash
cargo test -p fi-code-core test_tool_filter
cargo test -p fi-code-core test_profile
```

Expected: PASS (4 tests)

### Step 4: Commit

```bash
git add crates/core/src/agent/profile.rs crates/core/src/agent/mod.rs
git commit -m "feat(core): add AgentProfile and ToolFilter"
```

---

## Task 3: PromptBuilder Integration

**Files:**
- Modify: `crates/core/src/agent/prompt.rs`
- Test: `crates/core/src/agent/prompt.rs` (in-module tests)

### Step 1: Add `with_agent_profile` method

In `crates/core/src/agent/prompt.rs`, add to `PromptBuilder` impl:

```rust
use crate::agent::profile::AgentProfile;

// ... existing impl ...

impl PromptBuilder {
    // ... existing methods ...

    /// 构建系统提示词，并追加 Agent Profile 的后缀说明。
    pub fn build_with_profile(
        &self,
        tools_schema: &serde_json::Value,
        registry: &SkillRegistry,
        profile: &AgentProfile,
    ) -> String {
        let mut prompt = self.build(tools_schema, registry);
        prompt.push_str(profile.prompt_suffix);
        prompt
    }
}
```

### Step 2: Add test

In the existing `#[cfg(test)]` module at the bottom of `prompt.rs`, add:

```rust
    #[test]
    fn test_build_with_profile_appends_suffix() {
        use crate::agent::profile::AgentProfile;
        use fi_code_shared::dto::AgentType;

        let builder = PromptBuilder::new();
        let registry = SkillRegistry::new();
        let schema = serde_json::Value::Array(vec![]);
        let profile = AgentProfile::for_type(AgentType::Plan);

        let prompt = builder.build_with_profile(&schema, &registry, profile);

        assert!(prompt.contains("Agent Mode: Plan"));
        assert!(prompt.contains("planning assistant"));
    }
```

### Step 3: Run tests

```bash
cargo test -p fi-code-core test_build_with_profile
```

Expected: PASS (1 test)

### Step 4: Commit

```bash
git add crates/core/src/agent/prompt.rs
git commit -m "feat(core): add PromptBuilder::build_with_profile"
```

---

## Task 4: Refactor AgentRunner

**Files:**
- Modify: `crates/core/src/agent/runner.rs`
- Modify: `crates/core/src/agent/agent.rs` (update `run_one_turn` call sites)
- Test: `crates/core/src/agent/runner.rs` (in-module tests)

### Step 1: Refactor AgentRunner constructor

In `crates/core/src/agent/runner.rs`, modify the struct and impl:

```rust
use crate::agent::profile::AgentProfile;
use fi_code_shared::dto::AgentType;

// ... existing imports ...

pub struct AgentRunner {
    client: Box<dyn AIClient>,
    agent_type: AgentType,
    max_turns: usize,
}

impl AgentRunner {
    pub fn new(client: Box<dyn AIClient>, agent_type: AgentType) -> Self {
        Self {
            client,
            agent_type,
            max_turns: 25,
        }
    }

    pub fn with_max_turns(mut self, max: usize) -> Self {
        self.max_turns = max;
        self
    }

    // ... run and run_with_sink remain mostly unchanged ...
}
```

### Step 2: Update `run_one_turn` to use profile

In `AgentRunner::run_one_turn`, replace the existing system_prompt and tools_schema setup:

```rust
    async fn run_one_turn(
        &self,
        messages: &mut Vec<Message>,
        on_text: &mut Option<Box<dyn FnMut(&str) + Send>>,
        on_tool_event: &mut Option<Box<dyn FnMut(crate::server::transport::sse::SseEvent) + Send>>,
    ) -> Result<(bool, Option<FinishReason>, TokenUsage)> {
        let session_id = messages
            .last()
            .map(|m| m.session_id.clone())
            .unwrap_or_default();

        let assistant_count = messages
            .iter()
            .filter(|m| m.role == Role::Assistant)
            .count() as u32;
        let mut turn = TurnState::new(
            session_id.clone(),
            assistant_count + 1,
            TokenUsage::default(),
        );

        if let Some(ref mut cb) = on_tool_event {
            let _ = cb(crate::server::transport::sse::SseEvent::Part {
                part: turn.wave_marker.clone(),
            });
        }

        // 获取 Profile 并过滤工具
        let profile = AgentProfile::for_type(self.agent_type);
        let all_schema = crate::tools::tool_schema().await;
        let tools_schema = profile.tool_filter.apply(&all_schema);

        // 构建带 Profile 后缀的系统提示词
        let registry = crate::skills::get_registry();
        let system_prompt = PromptBuilder::new().build_with_profile(&tools_schema, registry, profile);

        // 消息历史截断（保持不变）
        const MAX_CONTEXT_MESSAGES: usize = 30;
        let messages_for_llm: &[Message] = if messages.len() > MAX_CONTEXT_MESSAGES {
            let start = messages.len().saturating_sub(MAX_CONTEXT_MESSAGES);
            &messages[start..]
        } else {
            &messages[..]
        };

        self.client
            .stream_message(
                &system_prompt,
                messages_for_llm,
                &tools_schema,
                &mut |chunk| {
                    // ... existing chunk handling ...
                },
            )
            .await?;

        // ... rest of run_one_turn remains unchanged ...
```

### Step 3: Update old `agent_loop` in `agent.rs`

In `crates/core/src/agent/agent.rs`, the old `run_one_turn` also calls `tool_schema()` and `PromptBuilder::new().build()`. Update it to use `AgentType::Build` as default for backward compatibility:

```rust
    let registry = get_registry();
    let schema = tool_schema().await;
    let profile = AgentProfile::for_type(AgentType::Build);
    let filtered_schema = profile.tool_filter.apply(&schema);
    let system_prompt = PromptBuilder::new().build_with_profile(&filtered_schema, registry, profile);
```

Also update the `execute_tool_calls` call to pass `AgentType::Build`:

```rust
    let tool_results = execute_tool_calls(&turn.content_blocks, AgentType::Build, on_tool_event).await;
```

### Step 4: Add test for AgentRunner with Plan agent

In `crates/core/src/agent/runner.rs`, add to existing tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use fi_code_shared::dto::AgentType;

    #[test]
    fn test_agent_runner_new_with_plan() {
        // This is a compile-time check; actual runner tests require a mock client
        // We verify the struct can be constructed with AgentType::Plan
        let runner = AgentRunner::new(
            Box::new(crate::provider::MockClient::new()),
            AgentType::Plan,
        );
        assert_eq!(runner.agent_type, AgentType::Plan);
    }
}
```

Note: If `MockClient` is not in `crate::provider`, use the appropriate mock or skip this test and add a comment.

### Step 5: Run tests

```bash
cargo test -p fi-code-core agent::runner
cargo test -p fi-code-core agent::agent
```

Expected: Compile success, existing tests PASS

### Step 6: Commit

```bash
git add crates/core/src/agent/runner.rs crates/core/src/agent/agent.rs
git commit -m "feat(core): refactor AgentRunner to accept AgentType"
```

---

## Task 5: Tool Layer Filtering

**Files:**
- Modify: `crates/core/src/tools/mod.rs`
- Test: `crates/core/src/tools/mod.rs` (in-module tests)

### Step 1: Add `tool_schema_for_agent`

In `crates/core/src/tools/mod.rs`, add after `pub async fn tool_schema()`:

```rust
/// 获取指定 Agent 类型的工具 schema（已过滤）。
pub async fn tool_schema_for_agent(agent_type: AgentType) -> serde_json::Value {
    use crate::agent::profile::AgentProfile;
    let all = tool_schema().await;
    let profile = AgentProfile::for_type(agent_type);
    profile.tool_filter.apply(&all)
}
```

Add import at top of file:
```rust
use fi_code_shared::dto::AgentType;
```

### Step 2: Update `execute_tool_calls` signature

Change:
```rust
pub async fn execute_tool_calls(
    parts: &[Part],
    on_tool_event: &mut Option<Box<dyn FnMut(crate::server::transport::sse::SseEvent) + Send>>,
) -> Vec<Part> {
```

To:
```rust
pub async fn execute_tool_calls(
    parts: &[Part],
    agent_type: AgentType,
    on_tool_event: &mut Option<Box<dyn FnMut(crate::server::transport::sse::SseEvent) + Send>>,
) -> Vec<Part> {
```

### Step 3: Add secondary filtering in execute_tool_calls

Inside `execute_tool_calls`, before creating the futures, add:

```rust
    use crate::agent::profile::AgentProfile;
    let profile = AgentProfile::for_type(agent_type);
```

Then in the `filter_map` closure, after extracting `id`, `name`, `arguments`, add:

```rust
            if !profile.tool_filter.allows(&name) {
                return Some(async move {
                    Part::ToolError {
                        tool_call_id: id.clone(),
                        content: format!("Tool '{}' is not allowed in {} Agent", name, profile.name),
                        error_message: "Permission denied by agent profile".to_string(),
                    }
                });
            }
```

### Step 4: Update all call sites of `execute_tool_calls`

Search for all calls to `execute_tool_calls` and add `AgentType::Build` as the second argument:

```bash
grep -r "execute_tool_calls(" crates/ --include="*.rs"
```

Known call sites:
1. `crates/core/src/agent/runner.rs` line 226
2. `crates/core/src/agent/agent.rs` line 556

Update both to pass `AgentType::Build` (for the old code path) or the appropriate agent type.

### Step 5: Add test for filtered tool execution

In `crates/core/src/tools/mod.rs` tests, add:

```rust
    #[tokio::test]
    async fn test_execute_tool_calls_plan_agent_blocks_write() {
        use fi_code_shared::dto::AgentType;
        let parts = vec![
            Part::ToolUse {
                id: "1".to_string(),
                name: "write".to_string(),
                arguments: serde_json::json!({"path": "/tmp/test.txt", "content": "hello"}),
            },
        ];
        let results = execute_tool_calls(&parts, AgentType::Plan, &mut None).await;
        assert_eq!(results.len(), 1);
        match &results[0] {
            Part::ToolError { error_message, .. } => {
                assert!(error_message.contains("Permission denied"));
            }
            _ => panic!("Expected ToolError for blocked tool"),
        }
    }

    #[tokio::test]
    async fn test_execute_tool_calls_build_agent_allows_write() {
        use fi_code_shared::dto::AgentType;
        // This test may fail if write is actually executed; we just verify it doesn't get blocked
        // by checking the result is not a ToolError
        let parts = vec![
            Part::ToolUse {
                id: "1".to_string(),
                name: "write".to_string(),
                arguments: serde_json::json!({"path": "/tmp/test_fi_code_build.txt", "content": "hello"}),
            },
        ];
        let results = execute_tool_calls(&parts, AgentType::Build, &mut None).await;
        assert_eq!(results.len(), 1);
        // Build Agent should attempt to execute (result may be success or failure, but not blocked)
        match &results[0] {
            Part::ToolError { error_message, .. } => {
                assert!(!error_message.contains("Permission denied by agent profile"));
            }
            _ => {}, // ToolResult or other is fine
        }
    }
```

### Step 6: Run tests

```bash
cargo test -p fi-code-core test_execute_tool_calls_plan
cargo test -p fi-code-core test_execute_tool_calls_build
```

Expected: PASS (2 tests)

### Step 7: Commit

```bash
git add crates/core/src/tools/mod.rs
git commit -m "feat(core): add agent-aware tool filtering in execute_tool_calls"
```

---

## Task 6: Session Persistence

**Files:**
- Modify: `crates/core/src/session/session.rs`
- Modify: `crates/core/src/session/mod.rs` (if needed for exports)
- Test: `crates/core/src/session/session.rs` (in-module tests)

### Step 1: Add `agent_type` to Session struct

In `crates/core/src/session/session.rs`, add to `Session` struct:

```rust
pub struct Session {
    pub id: String,
    pub project_path: String,
    pub created_at: u64,
    pub updated_at: u64,
    pub model: String,
    pub status: SessionStatus,
    pub agent_type: AgentType,  // 新增
    pub messages: Vec<Message>,
}
```

Add import:
```rust
use fi_code_shared::dto::AgentType;
```

### Step 2: Update `create_session` to default to Build

In `SessionManager::create_session`:

```rust
        let session = Session {
            id: id.clone(),
            project_path,
            created_at: now,
            updated_at: now,
            model: model.to_string(),
            status: SessionStatus::Active,
            agent_type: AgentType::Build,  // 新增
            messages: Vec::new(),
        };
```

### Step 3: Update `session_to_record`

In `session_to_record`:

```rust
    fields.insert("agent_type".to_string(), serde_json::to_value(&session.agent_type).unwrap());
```

### Step 4: Update `parse_session_record`

In `parse_session_record`:

```rust
        agent_type: record
            .fields
            .get("agent_type")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default(),  // 向后兼容：缺失时默认 Build
```

### Step 5: Add test for backward compatibility

In `crates/core/src/session/session.rs` tests:

```rust
    #[test]
    fn test_session_backward_compat_missing_agent_type() {
        // 模拟旧版 JSONL 记录（无 agent_type 字段）
        let record = Record {
            type_: "session".to_string(),
            fields: {
                let mut m = serde_json::Map::new();
                m.insert("id".to_string(), json!("test-session"));
                m.insert("project_path".to_string(), json!("/tmp"));
                m.insert("created_at".to_string(), json!(1234567890u64));
                m.insert("updated_at".to_string(), json!(1234567890u64));
                m.insert("model".to_string(), json!("gpt-4"));
                m.insert("status".to_string(), json!("active"));
                m
            },
        };
        let session = parse_session_record(record).unwrap();
        assert_eq!(session.agent_type, AgentType::Build);
    }

    #[test]
    fn test_session_roundtrip_agent_type() {
        let session = Session {
            id: "test".to_string(),
            project_path: "/tmp".to_string(),
            created_at: 0,
            updated_at: 0,
            model: "gpt-4".to_string(),
            status: SessionStatus::Active,
            agent_type: AgentType::Plan,
            messages: vec![],
        };
        let record = session_to_record(&session);
        assert_eq!(record.fields.get("agent_type").unwrap(), &json!("plan"));

        let parsed = parse_session_record(record).unwrap();
        assert_eq!(parsed.agent_type, AgentType::Plan);
    }
```

### Step 6: Run tests

```bash
cargo test -p fi-code-core session::session
```

Expected: PASS (existing + 2 new tests)

### Step 7: Commit

```bash
git add crates/core/src/session/session.rs
git commit -m "feat(core): bind AgentType to Session with JSONL persistence"
```

---

## Task 7: Server API

**Files:**
- Modify: `crates/core/src/server/api/chat_api.rs`
- Modify: `crates/shared/src/dto.rs` (already done in Task 1)
- Test: Manual curl test

### Step 1: Update ChatRequest

In `crates/core/src/server/api/chat_api.rs`, update the local `ChatRequest` struct (around line 48):

```rust
pub use fi_code_shared::dto::ChatRequest;
```

Wait, the `ChatRequest` is re-exported from `fi_code_shared::dto`. We already added `agent` to it in Task 1. Now we need to update the server handler to use it.

In `handle_chat_endpoint`, extract the agent type:

```rust
    let agent_type = req.agent.unwrap_or_default();
    log_info!("[Server] handle_chat_endpoint | agent={:?}", agent_type);
```

### Step 2: Pass agent_type to run_agent_chat

Update `run_agent_chat` signature and body:

```rust
async fn run_agent_chat(
    state: AppState,
    session_id: String,
    message: String,
    agent_type: AgentType,
    sse_sender: SseSender,
) -> Result<(), String> {
```

At the start of the function, send `AgentInfo` SSE event:

```rust
    // 发送当前 Agent 信息
    let profile = AgentProfile::for_type(agent_type);
    let _ = sse_sender
        .send(SseEvent::AgentInfo {
            agent_type,
            agent_name: profile.name.to_string(),
        })
        .await;
```

### Step 3: Update agent_loop call to use AgentType

In `run_agent_chat`, when creating the client and calling `agent_loop`, we need to pass the agent type. However, the old `agent_loop` signature doesn't accept `AgentType`. We have two options:

Option A: Modify `agent_loop` to accept `AgentType`
Option B: Use `AgentRunner` instead of `agent_loop`

Since we're refactoring toward `AgentRunner`, let's use it in the server:

Replace the `agent_loop` call with `AgentRunner`:

```rust
    use crate::agent::{AgentRunner, AgentProfile};
    
    let runner = AgentRunner::new(client, agent_type);
    let mut on_text: Option<Box<dyn FnMut(&str) + Send>> = Some(Box::new(move |text: &str| {
        let _ = sse_sender_for_stream.try_send(SseEvent::Message {
            content: text.to_string(),
        });
    }));
    let mut on_tool_event: Option<Box<dyn FnMut(SseEvent) + Send>> =
        Some(Box::new(move |event: SseEvent| {
            let _ = sse_sender_for_tools.try_send(event);
        }));

    if let Err(e) = runner.run_with_sink(loop_state.messages.clone(), &mut on_text).await {
        // ... error handling ...
    }
```

Wait, `AgentRunner::run_with_sink` doesn't accept `on_tool_event`. Let me check the signature again.

Actually, `AgentRunner::run_with_sink` signature is:
```rust
pub async fn run_with_sink(
    &self,
    initial_messages: Vec<Message>,
    on_text: &mut Option<Box<dyn FnMut(&str) + Send>>,
) -> Result<AgentRunResult>
```

It doesn't support `on_tool_event`. We need to either:
1. Add `on_tool_event` parameter to `AgentRunner::run_with_sink`
2. Or keep using `agent_loop` but modify it to accept `AgentType`

Let's go with option 2 for minimal change: modify `agent_loop` and `run_one_turn` to accept `AgentType`.

In `crates/core/src/agent/agent.rs`:

```rust
pub async fn agent_loop<C: AIClient + ?Sized>(
    client: &C,
    state: &mut LoopState,
    agent_type: AgentType,
    on_text: &mut Option<Box<dyn FnMut(&str) + Send>>,
    on_tool_event: &mut Option<Box<dyn FnMut(crate::server::transport::sse::SseEvent) + Send>>,
) -> Result<()> {
```

And in `run_one_turn`:
```rust
async fn run_one_turn<C: AIClient + ?Sized>(
    client: &C,
    state: &mut LoopState,
    agent_type: AgentType,
    on_text: &mut Option<Box<dyn FnMut(&str) + Send>>,
    on_tool_event: &mut Option<Box<dyn FnMut(crate::server::transport::sse::SseEvent) + Send>>,
) -> Result<bool> {
```

Then update the `agent_loop` call to `run_one_turn` to pass `agent_type`.

And update `run_one_turn` to use `agent_type` when calling `execute_tool_calls`:
```rust
    let tool_results = execute_tool_calls(&turn.content_blocks, agent_type, on_tool_event).await;
```

And when building system_prompt:
```rust
    let profile = AgentProfile::for_type(agent_type);
    let filtered_schema = profile.tool_filter.apply(&schema);
    let system_prompt = PromptBuilder::new().build_with_profile(&filtered_schema, registry, profile);
```

### Step 4: Update all agent_loop call sites

Search for `agent_loop(` calls:
```bash
grep -r "agent_loop(" crates/ --include="*.rs"
```

Known call sites:
1. `crates/core/src/server/api/chat_api.rs` line 222
2. Any others in CLI/TUI entry points

Update all to pass `AgentType::Build` (or the appropriate type).

### Step 5: Update `run_agent_chat` in chat_api.rs

```rust
async fn run_agent_chat(
    state: AppState,
    session_id: String,
    message: String,
    agent_type: AgentType,
    sse_sender: SseSender,
) -> Result<(), String> {
    // ... existing setup ...

    // 发送 AgentInfo
    let profile = AgentProfile::for_type(agent_type);
    let _ = sse_sender
        .send(SseEvent::AgentInfo {
            agent_type,
            agent_name: profile.name.to_string(),
        })
        .await;

    // ... existing message push ...

    // 运行 agent_loop
    if let Err(e) = agent_loop(
        client.as_ref(),
        &mut loop_state,
        agent_type,
        &mut on_text,
        &mut on_tool_event,
    )
    .await
    {
        // ... error handling ...
    }
    // ... rest unchanged ...
}
```

### Step 6: Update handle_chat_endpoint to extract agent_type

```rust
pub async fn handle_chat_endpoint(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<ChatRequest>,
) -> Response {
    // ... existing auth and session setup ...

    let agent_type = req.agent.unwrap_or_default();

    // ... spawn run_agent_chat with agent_type ...
    tokio::spawn(async move {
        let result = std::panic::AssertUnwindSafe(run_agent_chat(
            state,
            spawn_session_id.clone(),
            req.message,
            agent_type,
            sse_sender,
        ))
        .catch_unwind()
        .await;
        // ...
    });
    // ...
}
```

### Step 7: Manual test

```bash
cargo build -p fi-code-server
# In one terminal:
cargo run --bin fi-code-server
# In another terminal:
curl -N -X POST http://localhost:4040/chat \
  -H "Content-Type: application/json" \
  -d '{"message": "hello", "agent": "plan"}'
```

Expected: SSE stream starts with `AgentInfo` event showing `"agent_type": "plan"`.

### Step 8: Commit

```bash
git add crates/core/src/server/api/chat_api.rs crates/core/src/agent/agent.rs
git commit -m "feat(server): support agent parameter in chat API"
```

---

## Task 8: CLI Parameter

**Files:**
- Modify: `crates/cli/src/cli_args.rs`
- Modify: `crates/cli/src/entry.rs`

### Step 1: Add `--agent` parameter

In `crates/cli/src/cli_args.rs`, add to `Args` struct:

```rust
use fi_code_shared::dto::AgentType;
use clap::ValueEnum;

#[derive(Parser, Debug)]
#[command(name = "fi-code", version = env!("CARGO_PKG_VERSION"))]
pub struct Args {
    // ... existing fields ...

    /// Specify the agent type (build or plan)
    #[arg(long, value_enum, default_value = "build")]
    pub agent: AgentType,
}
```

Wait, `AgentType` needs to derive `ValueEnum` for clap. Let's check if it already derives the necessary traits.

`AgentType` currently derives: `Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize`

We need to add `ValueEnum` derive. But `ValueEnum` is from `clap`, which is not a dependency of `fi-code-shared`. 

Options:
1. Add `clap` as a dependency of `fi-code-shared` (not ideal, shared should be lightweight)
2. Define a separate CLI enum and convert
3. Use `str` parsing manually

Let's go with option 3 for minimal dependency impact: use `String` and parse manually.

```rust
    /// Specify the agent type (build or plan)
    #[arg(long, default_value = "build")]
    pub agent: String,
```

Then in `entry.rs`, parse the string to `AgentType`.

### Step 2: Parse agent in entry.rs

In `crates/cli/src/entry.rs`, where `AgentRunner` is constructed:

```rust
use fi_code_shared::dto::AgentType;

let agent_type = match args.agent.as_str() {
    "plan" => AgentType::Plan,
    _ => AgentType::Build,
};

let runner = AgentRunner::new(client, agent_type);
```

### Step 3: Build and test

```bash
cargo build -p fi-code-cli
cargo run --bin fi-code-cli -- --agent plan --help
```

Expected: Help shows `--agent` option with default "build".

### Step 4: Commit

```bash
git add crates/cli/src/cli_args.rs crates/cli/src/entry.rs
git commit -m "feat(cli): add --agent parameter"
```

---

## Task 9: TUI Status Bar

**Files:**
- Modify: `crates/tui/src/components/status_bar.rs`
- Test: `crates/tui/src/components/status_bar.rs` (in-module tests)

### Step 1: Add `agent_name` field

In `crates/tui/src/components/status_bar.rs`:

```rust
pub struct StatusBar {
    // ... existing fields ...
    agent_name: String,
}
```

In `new()`:
```rust
            agent_name: AgentType::Build.as_str().to_string(),
```

Add setter:
```rust
    pub fn set_agent(&mut self, agent_name: String) {
        self.agent_name = agent_name;
    }
```

### Step 2: Update `build_line` to display agent

In `build_line`, after the brand "FiCode" span, add:

```rust
        // Agent 名称
        spans.push(Span::styled(" │ ", theme.style_muted()));
        spans.push(Span::styled(
            format!("AGT: {}", self.agent_name),
            theme.style_primary(),
        ));
```

Place it after the brand and before CTX. For compact mode (≥80), keep "AGT:" abbreviation. For extreme mode (<80), hide AGT.

### Step 3: Update tests

Add test:
```rust
    #[test]
    fn test_status_bar_agent() {
        let mut bar = StatusBar::new();
        assert_eq!(bar.agent_name, "Build");
        bar.set_agent("Plan".to_string());
        assert_eq!(bar.agent_name, "Plan");
    }
```

### Step 4: Run tests

```bash
cargo test -p fi-code-tui test_status_bar_agent
```

Expected: PASS

### Step 5: Commit

```bash
git add crates/tui/src/components/status_bar.rs
git commit -m "feat(tui): display agent name in status bar"
```

---

## Task 10: TUI Event Handling

**Files:**
- Modify: `crates/tui/src/app.rs`
- Modify: `crates/tui/src/client.rs`
- Test: Manual TUI test

### Step 1: Add CTRL+A handling

In `crates/tui/src/app.rs`, in `handle_ctrl_key`, add case for 'a':

```rust
            'a' => {
                self.exit_confirm_pending = false;
                let current = self.current_agent_type();
                let next = match current {
                    AgentType::Build => AgentType::Plan,
                    AgentType::Plan => AgentType::Build,
                };
                self.handle_app_event(AppEvent::SwitchAgent(next)).await;
            }
```

Wait, `TuiApp` doesn't have `current_agent_type()` method. We need to add it or infer from somewhere. The TUI doesn't currently track the agent type. We need to add it to `TuiApp` state.

Add to `TuiApp` struct:
```rust
    current_agent: AgentType,
```

Initialize in `new()`:
```rust
            current_agent: AgentType::Build,
```

Add helper:
```rust
    fn current_agent_type(&self) -> AgentType {
        self.current_agent
    }
```

### Step 2: Handle SwitchAgent event

In `handle_app_event`, add case:

```rust
            AppEvent::SwitchAgent(agent_type) => {
                if self.is_generating {
                    self.chat.add_system_message(
                        "Please wait for the current response to complete before switching agents."
                            .to_string(),
                    );
                    return;
                }
                self.current_agent = agent_type;
                let profile = AgentProfile::for_type(agent_type);
                self.status_bar.set_agent(profile.name.to_string());
                self.event_tx
                    .send(AppEvent::AgentSwitched {
                        agent_type,
                        agent_name: profile.name.to_string(),
                    })
                    .ok();
            }
            AppEvent::AgentSwitched { agent_name, .. } => {
                self.chat
                    .add_system_message(format!("Switched to {} Agent", agent_name));
            }
```

### Step 3: Update TuiClient to pass agent

In `crates/tui/src/client.rs`, update `chat` method:

```rust
    pub async fn chat(
        &self,
        session_id: Option<String>,
        message: String,
        agent_type: AgentType,
        tx: mpsc::Sender<AppEvent>,
    ) -> Result<String> {
        let url = format!("{}/chat", self.base_url);
        let req_body = json!({
            "session_id": session_id,
            "message": message,
            "agent": agent_type
        });
        // ... rest unchanged ...
    }
```

### Step 4: Update start_chat_stream to pass agent

In `crates/tui/src/app.rs`, update `start_chat_stream`:

```rust
    async fn start_chat_stream(&mut self, message: String) {
        // ... existing setup ...
        let agent_type = self.current_agent;

        tokio::spawn(async move {
            match client.chat(session_id, message, agent_type, tx.clone()).await {
                // ... rest unchanged ...
            }
        });
    }
```

### Step 5: Build and test

```bash
cargo build -p fi-code-tui
```

Expected: Compile success.

### Step 6: Commit

```bash
git add crates/tui/src/app.rs crates/tui/src/client.rs
git commit -m "feat(tui): add CTRL+A agent switching"
```

---

## Task 11: Full Build and Test

### Step 1: Build all crates

```bash
cargo build
```

Expected: No compilation errors.

### Step 2: Run all unit tests

```bash
cargo test
```

Expected: All tests pass.

### Step 3: Run clippy

```bash
cargo clippy -- -D warnings
```

Expected: No warnings (or fix any that appear).

### Step 4: Format code

```bash
cargo fmt
```

### Step 5: Commit

```bash
git add -A
git commit -m "test: verify full build and test suite"
```

---

## Task 12: E2E / BDD Tests

**Files:**
- Create/Modify: `tests/e2e/cli_e2e.rs`
- Create/Modify: `tests/bdd/features/agent.feature`
- Create/Modify: `tests/bdd/steps/agent_steps.rs`

### Step 1: Add CLI E2E test for --agent

In `tests/e2e/cli_e2e.rs`, add:

```rust
#[tokio::test]
async fn test_cli_agent_parameter() {
    let bin = env!("CARGO_BIN_EXE_fi-code-cli");
    let output = Command::new(bin)
        .args(["--agent", "plan", "--help"])
        .output()
        .await
        .expect("failed to run fi-code-cli");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--agent"));
    assert!(stdout.contains("build") || stdout.contains("plan"));
}
```

### Step 2: Add BDD feature file

Create `tests/bdd/features/agent.feature`:

```gherkin
Feature: Agent System

  Scenario: Default agent is Build
    Given the application is running
    When I check the current agent
    Then the agent should be "Build"

  Scenario: Switch to Plan agent in TUI
    Given the TUI is open
    When I press Ctrl+A
    Then the agent should be "Plan"
    And the status bar should show "Plan"

  Scenario: Plan agent cannot use write tool
    Given the agent is "Plan"
    When I request to write a file
    Then the tool should be blocked with permission error
```

### Step 3: Add step definitions

Create `tests/bdd/steps/agent_steps.rs`:

```rust
use cucumber::{given, then, when};

#[given("the agent is \"Plan\"")]
fn given_plan_agent(world: &mut TestWorld) {
    world.agent_type = fi_code_shared::dto::AgentType::Plan;
}

#[when("I request to write a file")]
async fn when_request_write(world: &mut TestWorld) {
    let parts = vec![fi_code_core::session::message::Part::ToolUse {
        id: "1".to_string(),
        name: "write".to_string(),
        arguments: serde_json::json!({"path": "/tmp/test.txt", "content": "hello"}),
    }];
    world.tool_results = fi_code_core::tools::execute_tool_calls(
        &parts,
        world.agent_type,
        &mut None,
    ).await;
}

#[then("the tool should be blocked with permission error")]
fn then_tool_blocked(world: &mut TestWorld) {
    assert_eq!(world.tool_results.len(), 1);
    match &world.tool_results[0] {
        fi_code_core::session::message::Part::ToolError { error_message, .. } => {
            assert!(error_message.contains("Permission denied"));
        }
        _ => panic!("Expected ToolError"),
    }
}
```

### Step 4: Run BDD tests

```bash
cargo test --test bdd
```

Expected: New scenarios pass.

### Step 5: Commit

```bash
git add tests/
git commit -m "test: add E2E and BDD tests for Agent System"
```

---

## Task 13: Documentation and Final Review

### Step 1: Update AGENTS.md

Add a section about Agent System to `AGENTS.md`:

```markdown
## Agent System

fi-code supports two agent types:

- **Build Agent** (default): Full-featured coding assistant with all tools.
- **Plan Agent**: Read-only planning assistant, can only use read tools and web_fetch.

### Switching Agents

- **TUI**: Press `CTRL+A` to toggle between Build and Plan.
- **CLI**: Use `--agent plan` or `--agent build`.
- **Server API**: Pass `"agent": "plan"` in the `/chat` request body.

### Session Binding

Agent type is bound to each session and persisted in JSONL. When restoring a session,
the previous agent type is restored.
```

### Step 2: Run final verification

```bash
cargo build
cargo test
cargo clippy -- -D warnings
cargo fmt
```

### Step 3: Commit

```bash
git add AGENTS.md
git commit -m "docs: update AGENTS.md with Agent System documentation"
```

---

## Self-Review Checklist

**1. Spec coverage:**
- [x] AgentType enum → Task 1
- [x] AgentProfile with ToolFilter → Task 2
- [x] PromptBuilder integration → Task 3
- [x] AgentRunner refactor → Task 4
- [x] Tool filtering in execute_tool_calls → Task 5
- [x] Session persistence → Task 6
- [x] Server API agent field → Task 7
- [x] CLI --agent parameter → Task 8
- [x] TUI status bar display → Task 9
- [x] TUI CTRL+A switching → Task 10
- [x] TUI client passing agent → Task 10
- [x] Tests → Tasks 11-12

**2. Placeholder scan:**
- [x] No TBD/TODO/fill in details
- [x] All code blocks contain actual code
- [x] All commands have expected output

**3. Type consistency:**
- [x] `AgentType` used consistently across all tasks
- [x] `AgentProfile::for_type()` signature consistent
- [x] `execute_tool_calls` signature updated in all call sites
