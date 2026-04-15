# Session Persistence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement full session persistence with JSONL storage, multi-session management, and resume capability for the shun-code CLI.

**Architecture:** Replace existing `agent::Message`/`ContentBlock` with design-doc-aligned `Message`/`Part` types. Implement a synchronous `SessionManager` in `src/session/` that reads/writes append-only JSONL. Wire session creation, loading, and saving into `main.rs`'s REPL loop.

**Tech Stack:** Rust, `ulid`, `directories`, `serde_json`, `tokio` (for `spawn_blocking`)

---

## File Structure

| File | Responsibility |
|------|----------------|
| `Cargo.toml` | Add `ulid` and `directories` dependencies |
| `src/agent/mod.rs` | Define new `Message`, `Role`, `Part`, `ImageSource` types; update `LoopState`; keep `agent_loop` signature stable |
| `src/provider/base_client.rs` | Update `stream_message` to accept `&[Message]` (type unchanged, but internal serialization adapts to new `Part`) |
| `src/provider/client/anthropic_client.rs` | Map SSE chunks to new `Part` enum; update request body serialization |
| `src/provider/client/openapi_client.rs` | Map SSE chunks to new `Part` enum; update request body serialization |
| `src/provider/mod.rs` | Update `extract_text` to consume `&[Part]` instead of `&[ContentBlock]` |
| `src/tools/mod.rs` | Change `execute_tool_calls` to return `Vec<Part::ToolResult>`; update `tool_call` return types |
| `src/tools/basic_tools.rs` | Update handlers to return `Result<String, String>` (unchanged, called by `tool_call`) |
| `src/session/mod.rs` | Define `Session`, `SessionStatus`, `SessionMeta`, `SessionManager`; implement JSONL read/write |
| `src/main.rs` | Integrate `SessionManager`: create/load sessions, append messages after each turn, update prompt prefix |

---

## Task 1: Add Dependencies

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Add `ulid` and `directories` to Cargo.toml**

```toml
[dependencies]
# ... existing deps ...
ulid = "1.1"
directories = "5.0"
```

- [ ] **Step 2: Verify Cargo.lock updates**

Run: `cargo check`
Expected: Dependencies resolve successfully; no compilation yet (code still broken later).

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "deps: add ulid and directories for session persistence"
```

---

## Task 2: Refactor Agent Types (Message / Part)

**Files:**
- Modify: `src/agent/mod.rs`

- [ ] **Step 1: Replace existing types with new Message / Part / Role**

Delete the old `Message` and `ContentBlock` definitions. Replace with:

```rust
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub session_id: String,
    pub role: Role,
    pub created_at: u64,
    pub parts: Vec<Part>,
    pub token_count: Option<u64>,
    pub cost: Option<f64>,
}

impl Message {
    pub fn new(session_id: impl Into<String>, role: Role, parts: Vec<Part>) -> Self {
        Self {
            id: ulid::Ulid::new().to_string(),
            session_id: session_id.into(),
            role,
            created_at: current_timestamp_ms(),
            parts,
            token_count: None,
            cost: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
    System,
    Developer,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Part {
    Text { text: String },
    Image { source: ImageSource },
    ToolUse { id: String, name: String, arguments: serde_json::Value },
    ToolResult { tool_call_id: String, content: String, is_error: bool },
    Reasoning { thinking: String, signature: Option<String> },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ImageSource {
    Path { path: String },
    Base64 { media_type: String, data: String },
    Url { url: String },
}

fn current_timestamp_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
```

- [ ] **Step 2: Update LoopState to use new Message type**

`LoopState` already stores `Vec<Message>` so its struct definition stays identical (the type name is the same). No changes needed to `LoopState`.

- [ ] **Step 3: Update agent_loop internals to build Part instead of ContentBlock**

In `run_one_turn` and `agent_loop`, replace all `ContentBlock::Text` with `Part::Text`, `ContentBlock::Think` with `Part::Reasoning`, `ContentBlock::ToolUse` with `Part::ToolUse`.

For example, change:
```rust
ContentBlock::Text { text: chunk_text }
```
to:
```rust
Part::Text { text: chunk_text }
```

Change:
```rust
ContentBlock::Think { text: chunk_text }
```
to:
```rust
Part::Reasoning { thinking: chunk_text, signature: None }
```

Change:
```rust
ContentBlock::ToolUse { id, name, input }
```
to:
```rust
Part::ToolUse { id, name, arguments: input }
```

- [ ] **Step 4: Update tool result message construction in agent_loop**

In `agent_loop`, after `execute_tool_calls`, wrap results into a `Message`:

```rust
let tool_results = execute_tool_calls(&content_blocks);
if !tool_results.is_empty() {
    let tool_msg = Message::new(
        "SESSION_ID_PLACEHOLDER", // will be passed from main.rs later
        Role::User,
        tool_results,
    );
    state.messages.push(tool_msg);
}
```

For now use a placeholder string; Task 8 will wire the real session ID.

- [ ] **Step 5: Verify agent module compiles in isolation**

Run: `cargo check --lib`
Expected: This will fail because other modules reference old types; that's expected. At minimum verify the file parses.

- [ ] **Step 6: Commit**

```bash
git add src/agent/mod.rs
git commit -m "refactor(agent): replace Message/ContentBlock with design-doc Part types"
```

---

## Task 3: Update Provider Mod (extract_text)

**Files:**
- Modify: `src/provider/mod.rs`

- [ ] **Step 1: Update extract_text to use &[Part]**

Replace the existing `extract_text` implementation with:

```rust
use crate::agent::Part;

pub fn extract_text(parts: &[Part]) -> String {
    parts
        .iter()
        .filter_map(|block| match block {
            Part::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("")
}
```

- [ ] **Step 2: Update exports if necessary**

Remove any `ContentBlock` re-exports if present. Ensure `mod.rs` exports `extract_text` and provider/client types only.

- [ ] **Step 3: Commit**

```bash
git add src/provider/mod.rs
git commit -m "refactor(provider): update extract_text for new Part type"
```

---

## Task 4: Update Anthropic Client for New Types

**Files:**
- Modify: `src/provider/client/anthropic_client.rs`

- [ ] **Step 1: Update request body serialization to use Part**

In `build_request_body`, the `messages` loop that serializes each `Message` needs to adapt to `Vec<Part>`.

Anthropic expects messages as `{ "role": "...", "content": [...] }` where content is an array of content blocks. For `Part::Text`, emit `{"type":"text","text":"..."}`. For `Part::Image` with `ImageSource::Base64`, emit `{"type":"image","source":{"type":"base64",...}}`. For `Part::ToolUse`, emit `{"type":"tool_use","id":"...","name":"...","input":{...}}`. For `Part::ToolResult`, emit `{"type":"tool_result","tool_use_id":"...","content":"...","is_error":...}`.

Replace the existing `match message.content` block with a loop over `message.parts`:

```rust
let mut content = Vec::new();
for part in &message.parts {
    let value = match part {
        crate::agent::Part::Text { text } => {
            json!({"type": "text", "text": text})
        }
        crate::agent::Part::Image { source } => match source {
            crate::agent::ImageSource::Base64 { media_type, data } => {
                json!({
                    "type": "image",
                    "source": {
                        "type": "base64",
                        "media_type": media_type,
                        "data": data
                    }
                })
            }
            _ => {
                // For now, Path/Url images are not supported in Anthropic path; skip or error
                continue;
            }
        }
        crate::agent::Part::ToolUse { id, name, arguments } => {
            json!({
                "type": "tool_use",
                "id": id,
                "name": name,
                "input": arguments
            })
        }
        crate::agent::Part::ToolResult { tool_call_id, content: c, is_error } => {
            json!({
                "type": "tool_result",
                "tool_use_id": tool_call_id,
                "content": c,
                "is_error": is_error
            })
        }
        crate::agent::Part::Reasoning { thinking, .. } => {
            // Anthropic extended thinking may use a different block type;
            // for now map to text to preserve content
            json!({"type": "text", "text": thinking})
        }
    };
    content.push(value);
}
req_messages.push(json!({"role": message.role, "content": content}));
```

Note: `message.role` is now a `Role` enum; it serializes to `"user"`, `"assistant"`, etc. via serde. You may need to convert to lowercase string manually if serde doesn't auto-serialize inline:

```rust
let role_str = match message.role {
    crate::agent::Role::User => "user",
    crate::agent::Role::Assistant => "assistant",
    crate::agent::Role::System => "system",
    crate::agent::Role::Developer => "developer",
};
```

Use `role_str` in the JSON.

- [ ] **Step 2: Update SSE chunk aggregation to produce Part**

In the SSE parsing loop, replace:
- `ChunkContent::Text(text)` aggregation target → produces `Part::Text`
- `ChunkContent::Think(text)` → produces `Part::Reasoning { thinking: text, signature: None }`
- `ChunkContent::ToolUse(ContentBlock { ... })` → produces `Part::ToolUse { id, name, arguments }`

Update the final `content_blocks` type annotation from `Vec<ContentBlock>` to `Vec<Part>`.

- [ ] **Step 3: Commit**

```bash
git add src/provider/client/anthropic_client.rs
git commit -m "refactor(provider): adapt anthropic client to new Message/Part types"
```

---

## Task 5: Update OpenAI Client for New Types

**Files:**
- Modify: `src/provider/client/openapi_client.rs`

- [ ] **Step 1: Update request body serialization to use Part**

OpenAI expects `content` as either a string or an array of content parts. Use array format for consistency.

In the message serialization loop:

```rust
let mut content_parts = Vec::new();
for part in &message.parts {
    match part {
        crate::agent::Part::Text { text } => {
            content_parts.push(json!({"type": "text", "text": text}));
        }
        crate::agent::Part::Image { source } => match source {
            crate::agent::ImageSource::Url { url } => {
                content_parts.push(json!({
                    "type": "image_url",
                    "image_url": { "url": url }
                }));
            }
            crate::agent::ImageSource::Base64 { media_type, data } => {
                content_parts.push(json!({
                    "type": "image_url",
                    "image_url": {
                        "url": format!("data:{};base64, {}", media_type, data)
                    }
                }));
            }
            _ => continue,
        }
        crate::agent::Part::ToolUse { id, name, arguments } => {
            // ToolUse is handled via top-level tool_calls, not content array
        }
        crate::agent::Part::ToolResult { tool_call_id, content: c, is_error } => {
            content_parts.push(json!({
                "type": "text",
                "text": format!("tool_result: {} {}", tool_call_id, c)
            }));
        }
        crate::agent::Part::Reasoning { thinking, .. } => {
            content_parts.push(json!({"type": "text", "text": thinking}));
        }
    }
}
```

For `ToolUse`, OpenAI uses a top-level `tool_calls` array on the assistant message. Tool results go as user messages with `tool_call_id` and `role: tool`. When reading `Message` from our `Part` model:
- If a message contains `Part::ToolUse`, extract them into `tool_calls` for assistant messages.
- If a message contains `Part::ToolResult`, serialize as `{ "role": "tool", "tool_call_id": "...", "content": "..." }`.

Do the minimal mapping to keep tests passing. If existing OpenAI serialization already uses `function` wrapping, preserve that but adapt the source fields from `Part::ToolUse`.

- [ ] **Step 2: Update SSE chunk aggregation to produce Part**

Same as Anthropic: map streaming text to `Part::Text`, tool_calls to `Part::ToolUse`, reasoning (if present in delta) to `Part::Reasoning`.

- [ ] **Step 3: Commit**

```bash
git add src/provider/client/openapi_client.rs
git commit -m "refactor(provider): adapt openai client to new Message/Part types"
```

---

## Task 6: Update Tools Layer

**Files:**
- Modify: `src/tools/mod.rs`

- [ ] **Step 1: Change execute_tool_calls to return Vec<Part::ToolResult>**

Replace the return type of `execute_tool_calls`:

```rust
pub fn execute_tool_calls(tool_calls: &[Part]) -> Vec<Part> {
    let mut results = Vec::new();
    for call in tool_calls {
        if let Part::ToolUse { id, name, arguments } = call {
            println!("{} {}", "Calling tool:".yellow(), name);
            let params = parse_tool_params(arguments.clone());
            let result = tool_call(name, params);
            let (content, is_error) = match result {
                Ok(output) => (output, false),
                Err(err) => (err, true),
            };
            results.push(Part::ToolResult {
                tool_call_id: id.clone(),
                content,
                is_error,
            });
        }
    }
    results
}
```

Delete the old `serde_json::Value`-based result construction.

- [ ] **Step 2: Update any helper that built JSON tool results**

Remove the old `json!({ "type": "tool_result", ... })` code since `Part::ToolResult` now carries the data structurally.

- [ ] **Step 3: Commit**

```bash
git add src/tools/mod.rs
git commit -m "refactor(tools): return Part::ToolResult instead of raw JSON"
```

---

## Task 7: Implement Session Manager and JSONL I/O

**Files:**
- Create: `src/session/mod.rs`

- [ ] **Step 1: Create src/session/mod.rs with Session types**

```rust
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::agent::{Message, Part, Role};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    Active,
    Idle,
    Archived,
}

#[derive(Clone, Debug)]
pub struct Session {
    pub id: String,
    pub project_path: String,
    pub created_at: u64,
    pub updated_at: u64,
    pub model: String,
    pub status: SessionStatus,
    pub messages: Vec<Message>,
}

#[derive(Clone, Debug)]
pub struct SessionMeta {
    pub id: String,
    pub project_path: String,
    pub created_at: u64,
    pub updated_at: u64,
    pub model: String,
    pub status: SessionStatus,
    pub message_count: usize,
}

pub struct SessionManager {
    sessions_dir: PathBuf,
}

impl SessionManager {
    pub fn new(sessions_dir: PathBuf) -> Self {
        Self { sessions_dir }
    }

    pub fn create_session(&self, model: &str) -> Result<Session> {
        fs::create_dir_all(&self.sessions_dir)?;
        let id = ulid::Ulid::new().to_string();
        let now = current_timestamp_ms();
        let project_path = std::env::current_dir()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let session = Session {
            id: id.clone(),
            project_path,
            created_at: now,
            updated_at: now,
            model: model.to_string(),
            status: SessionStatus::Active,
            messages: Vec::new(),
        };
        self.write_session_header(&session)?;
        Ok(session)
    }

    pub fn list_sessions(&self) -> Result<Vec<SessionMeta>> {
        let mut metas = Vec::new();
        if !self.sessions_dir.exists() {
            return Ok(metas);
        }
        for entry in fs::read_dir(&self.sessions_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("jsonl") {
                if let Some(id) = path.file_stem().and_then(|s| s.to_str()) {
                    if let Ok(session) = self.load_session(id) {
                        metas.push(SessionMeta {
                            id: session.id,
                            project_path: session.project_path,
                            created_at: session.created_at,
                            updated_at: session.updated_at,
                            model: session.model,
                            status: session.status,
                            message_count: session.messages.len(),
                        });
                    }
                }
            }
        }
        metas.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(metas)
    }

    pub fn load_session(&self, session_id: &str) -> Result<Session> {
        let path = self.session_path(session_id);
        let file = File::open(&path)
            .with_context(|| format!("Failed to open session file: {:?}", path))?;
        let reader = BufReader::new(file);

        let mut session: Option<Session> = None;
        let mut current_message: Option<MessageBuilder> = None;

        for (line_no, line) in reader.lines().enumerate() {
            let line = match line {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("Warning: failed to read line {}: {}", line_no + 1, e);
                    continue;
                }
            };
            let record: Record = match serde_json::from_str(&line) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Warning: failed to parse line {}: {}", line_no + 1, e);
                    continue;
                }
            };

            match record.type_.as_str() {
                "session" => {
                    session = Some(parse_session_record(record)?);
                }
                "message_start" => {
                    current_message = Some(MessageBuilder::new(record)?);
                }
                "part" => {
                    if let Some(ref mut builder) = current_message {
                        builder.add_part(record)?;
                    }
                }
                "message_end" => {
                    if let Some(builder) = current_message.take() {
                        let msg = builder.finalize(record)?;
                        if let Some(ref mut s) = session {
                            s.messages.push(msg);
                        }
                    }
                }
                _ => {
                    eprintln!("Warning: unknown record type on line {}", line_no + 1);
                }
            }
        }

        session.with_context(|| format!("No session header found in {:?}", path))
    }

    pub fn save_session(&self, session: &Session) -> Result<()> {
        fs::create_dir_all(&self.sessions_dir)?;
        let path = self.session_path(&session.id);
        let mut file = File::create(&path)?;
        writeln!(file, "{}", serde_json::to_string(&session_to_record(session))?)?;
        for msg in &session.messages {
            self.write_message(&mut file, msg)?;
        }
        Ok(())
    }

    pub fn append_message(&self, session_id: &str, message: &Message) -> Result<()> {
        let path = self.session_path(session_id);
        let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
        self.write_message(&mut file, message)?;
        Ok(())
    }

    fn write_session_header(&self, session: &Session) -> Result<()> {
        let path = self.session_path(&session.id);
        let mut file = File::create(&path)?;
        writeln!(file, "{}", serde_json::to_string(&session_to_record(session))?)?;
        Ok(())
    }

    fn write_message(&self, file: &mut File, message: &Message) -> Result<()> {
        writeln!(
            file,
            "{}",
            serde_json::to_string(&message_start_record(message))?
        )?;
        for (seq, part) in message.parts.iter().enumerate() {
            writeln!(
                file,
                "{}",
                serde_json::to_string(&part_record(message, seq, part))?
            )?;
        }
        writeln!(
            file,
            "{}",
            serde_json::to_string(&message_end_record(message))?
        )?;
        Ok(())
    }

    fn session_path(&self, session_id: &str) -> PathBuf {
        self.sessions_dir.join(format!("{}.jsonl", session_id))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Record {
    #[serde(rename = "type")]
    type_: String,
    #[serde(flatten)]
    fields: serde_json::Map<String, serde_json::Value>,
}

fn session_to_record(session: &Session) -> Record {
    let mut fields = serde_json::Map::new();
    fields.insert("id".to_string(), json!(session.id));
    fields.insert("project_path".to_string(), json!(session.project_path));
    fields.insert("created_at".to_string(), json!(session.created_at));
    fields.insert("updated_at".to_string(), json!(session.updated_at));
    fields.insert("model".to_string(), json!(session.model));
    fields.insert("status".to_string(), json!(session.status));
    Record {
        type_: "session".to_string(),
        fields,
    }
}

fn parse_session_record(record: Record) -> Result<Session> {
    Ok(Session {
        id: get_str(&record, "id")?,
        project_path: get_str(&record, "project_path")?,
        created_at: get_u64(&record, "created_at")?,
        updated_at: get_u64(&record, "updated_at")?,
        model: get_str(&record, "model")?,
        status: serde_json::from_value(
            record.fields.get("status").cloned().unwrap_or(json!("active"))
        )?,
        messages: Vec::new(),
    })
}

struct MessageBuilder {
    id: String,
    session_id: String,
    role: Role,
    created_at: u64,
    parts: Vec<Part>,
    token_count: Option<u64>,
    cost: Option<f64>,
}

impl MessageBuilder {
    fn new(record: Record) -> Result<Self> {
        let role_str = get_str(&record, "role")?;
        Ok(Self {
            id: get_str(&record, "message_id")?,
            session_id: get_str(&record, "session_id").unwrap_or_default(),
            role: serde_json::from_value(json!(role_str))?,
            created_at: get_u64(&record, "created_at")?,
            parts: Vec::new(),
            token_count: None,
            cost: None,
        })
    }

    fn add_part(&mut self, record: Record) -> Result<()> {
        let part_value = record
            .fields
            .get("part")
            .cloned()
            .context("Missing 'part' field")?;
        let part: Part = serde_json::from_value(part_value)?;
        self.parts.push(part);
        Ok(())
    }

    fn finalize(self, record: Record) -> Result<Message> {
        Ok(Message {
            id: self.id,
            session_id: self.session_id,
            role: self.role,
            created_at: self.created_at,
            parts: self.parts,
            token_count: record.fields.get("token_count").and_then(|v| v.as_u64()),
            cost: record.fields.get("cost").and_then(|v| v.as_f64()),
        })
    }
}

fn message_start_record(message: &Message) -> Record {
    let mut fields = serde_json::Map::new();
    fields.insert("message_id".to_string(), json!(message.id));
    fields.insert("session_id".to_string(), json!(message.session_id));
    fields.insert(
        "role".to_string(),
        serde_json::to_value(&message.role).unwrap(),
    );
    fields.insert("created_at".to_string(), json!(message.created_at));
    Record {
        type_: "message_start".to_string(),
        fields,
    }
}

fn part_record(message: &Message, sequence: usize, part: &Part) -> Record {
    let mut fields = serde_json::Map::new();
    fields.insert("message_id".to_string(), json!(message.id));
    fields.insert("sequence".to_string(), json!(sequence));
    fields.insert("part".to_string(), serde_json::to_value(part).unwrap());
    Record {
        type_: "part".to_string(),
        fields,
    }
}

fn message_end_record(message: &Message) -> Record {
    let mut fields = serde_json::Map::new();
    fields.insert("message_id".to_string(), json!(message.id));
    if let Some(tc) = message.token_count {
        fields.insert("token_count".to_string(), json!(tc));
    }
    if let Some(c) = message.cost {
        fields.insert("cost".to_string(), json!(c));
    }
    Record {
        type_: "message_end".to_string(),
        fields,
    }
}

fn get_str(record: &Record, key: &str) -> Result<String> {
    record
        .fields
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .with_context(|| format!("Missing or invalid field: {}", key))
}

fn get_u64(record: &Record, key: &str) -> Result<u64> {
    record
        .fields
        .get(key)
        .and_then(|v| v.as_u64())
        .with_context(|| format!("Missing or invalid field: {}", key))
}

fn current_timestamp_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

// Re-export json! macro usage helper
#[macro_use]
extern crate serde_json;
```

Note: The `#[macro_use] extern crate serde_json;` line should NOT be added if `serde_json` is already globally available via 2021 edition prelude. Instead, use `serde_json::json!` directly or import it with `use serde_json::json;` at the top of the file.

- [ ] **Step 2: Add tests for SessionManager**

Append to `src/session/mod.rs` inside a `#[cfg(test)]` module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{Part, Role};
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_create_and_load_session() {
        let dir = TempDir::new().unwrap();
        let manager = SessionManager::new(dir.path().to_path_buf());
        let session = manager.create_session("claude-test").unwrap();
        assert_eq!(session.model, "claude-test");
        assert!(session.messages.is_empty());

        let loaded = manager.load_session(&session.id).unwrap();
        assert_eq!(loaded.id, session.id);
        assert_eq!(loaded.model, session.model);
    }

    #[test]
    fn test_append_and_load_message() {
        let dir = TempDir::new().unwrap();
        let manager = SessionManager::new(dir.path().to_path_buf());
        let session = manager.create_session("gpt-test").unwrap();

        let msg = Message {
            id: "msg-001".to_string(),
            session_id: session.id.clone(),
            role: Role::User,
            created_at: 1234567890000,
            parts: vec![Part::Text {
                text: "hello world".to_string(),
            }],
            token_count: Some(2),
            cost: Some(0.001),
        };

        manager.append_message(&session.id, &msg).unwrap();

        let loaded = manager.load_session(&session.id).unwrap();
        assert_eq!(loaded.messages.len(), 1);
        assert_eq!(loaded.messages[0].id, "msg-001");
        assert_eq!(loaded.messages[0].parts.len(), 1);
        match &loaded.messages[0].parts[0] {
            Part::Text { text } => assert_eq!(text, "hello world"),
            _ => panic!("Expected Text part"),
        }
    }

    #[test]
    fn test_corrupted_line_skip() {
        let dir = TempDir::new().unwrap();
        let manager = SessionManager::new(dir.path().to_path_buf());
        let session = manager.create_session("test").unwrap();

        // Manually append a corrupted line
        let path = dir.path().join(format!("{}.jsonl", session.id));
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        writeln!(file, "this is not json").unwrap();

        let loaded = manager.load_session(&session.id).unwrap();
        assert_eq!(loaded.messages.len(), 0); // should still load session header
    }
}
```

- [ ] **Step 3: Add tempfile dev-dependency if not present**

Check `Cargo.toml` dev-dependencies. If `tempfile` is not there, add it:

```toml
[dev-dependencies]
wiremock = "0.6"
tempfile = "3.10"
```

- [ ] **Step 4: Run session tests**

Run: `cargo test --lib session`
Expected: All 3 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/session/mod.rs Cargo.toml Cargo.lock
git commit -m "feat(session): implement SessionManager with JSONL read/write and tests"
```

---

## Task 8: Integrate Session Management into Main.rs

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Import session types and update main flow**

Add to imports:
```rust
use session::{SessionManager, SessionMeta};
use agent::Role;
```

Before the REPL loop, initialize `SessionManager` and decide whether to create or resume a session:

```rust
#[tokio::main]
async fn main() -> Result<()> {
    let model = Model::get_model()?;
    let mut provider = Provider::new();
    provider.set_model(model.clone());
    let client = provider.get_client()?;
    let mut editor = DefaultEditor::new()?;

    let config_dir = directories::ProjectDirs::from("", "", "shun-code")
        .map(|d| d.config_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from(".config/shun-code"));
    let sessions_dir = config_dir.join("sessions");
    let session_manager = SessionManager::new(sessions_dir.clone());

    let mut session = choose_or_create_session(&session_manager, &model.model_name).await?;
    let prompt_prefix = format!("{} >> ", &session.id[..8]);

    loop {
        let readline = editor.readline(prompt_prefix.cyan().to_string().as_str());

        match readline {
            Ok(line) => {
                let query = line.trim();
                if query.is_empty() || ["q", "exit"].contains(&query.to_lowercase().as_str()) {
                    break;
                }
                editor.add_history_entry(query)?;

                let user_msg = agent::Message::new(
                    session.id.clone(),
                    Role::User,
                    vec![agent::Part::Text { text: query.to_string() }],
                );
                session.messages.push(user_msg.clone());

                if let Err(e) = session_manager.append_message(&session.id, &user_msg) {
                    eprintln!("Warning: failed to persist user message: {}", e);
                }

                let mut state = LoopState::new(session.messages.clone());
                agent_loop(client.as_ref(), &mut state).await?;
                session.messages = state.messages;

                // Persist any new messages appended during agent_loop (assistant + tool results)
                // For simplicity, save the whole session after each turn
                if let Err(e) = tokio::task::spawn_blocking({
                    let sm = SessionManager::new(sessions_dir.clone());
                    let s = session.clone();
                    move || sm.save_session(&s)
                }).await? {
                    eprintln!("Warning: failed to save session: {}", e);
                }

                if let Some(last_msg) = session.messages.last() {
                    if last_msg.role == Role::Assistant {
                        let text = provider::extract_text(&last_msg.parts);
                        if !text.is_empty() {
                            println!("{}", text);
                        }
                    }
                    println!();
                }
            }
            Err(rustyline::error::ReadlineError::Interrupted)
            | Err(rustyline::error::ReadlineError::Eof) => {
                break;
            }
            Err(err) => {
                eprintln!("Error: {:?}", err);
                break;
            }
        }
    }

    Ok(())
}
```

- [ ] **Step 2: Add session selection helper**

Append to `src/main.rs`:

```rust
async fn choose_or_create_session(
    manager: &SessionManager,
    model_name: &str,
) -> Result<session::Session> {
    let sessions = manager.list_sessions()?;
    if sessions.is_empty() {
        return Ok(manager.create_session(model_name)?);
    }

    println!("Recent sessions:");
    for (i, s) in sessions.iter().enumerate() {
        println!(
            "  [{}] {} | {} | {} messages | {}",
            i + 1,
            &s.id[..8],
            s.project_path,
            s.message_count,
            if s.status == session::SessionStatus::Active {
                "active"
            } else {
                "archived"
            }
        );
    }
    println!("  [0] Create new session");
    println!();
    print!("Select session [1]: ");
    use std::io::Write;
    std::io::stdout().flush()?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let choice = input.trim().parse::<usize>().unwrap_or(1);

    if choice == 0 {
        Ok(manager.create_session(model_name)?)
    } else if choice <= sessions.len() {
        Ok(manager.load_session(&sessions[choice - 1].id)?)
    } else {
        Ok(manager.load_session(&sessions[0].id)?)
    }
}
```

- [ ] **Step 3: Fix agent_loop placeholder session_id**

Go back to `src/agent/mod.rs` and remove the `"SESSION_ID_PLACEHOLDER"` string. Instead, pass the actual session ID from the outer context. Since `agent_loop` receives `state` which already contains `messages` with correct `session_id`s, the tool result message should inherit the session ID from the last user message:

```rust
let session_id = state
    .messages
    .last()
    .map(|m| m.session_id.clone())
    .unwrap_or_default();
let tool_msg = Message::new(session_id, Role::User, tool_results);
```

- [ ] **Step 4: Verify full compilation**

Run: `cargo check`
Expected: Zero errors.

- [ ] **Step 5: Run tests**

Run: `cargo test`
Expected: All existing + new tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/main.rs src/agent/mod.rs
git commit -m "feat(main): integrate session manager, support create/resume sessions"
```

---

## Self-Review Checklist

- [ ] **Spec coverage:** Every section of `docs/session-desgin.md` is addressed by at least one task.
  - Data model (Task 2) ✓
  - JSONL format (Task 7) ✓
  - SessionManager API (Task 7) ✓
  - Provider adaptation (Tasks 4, 5) ✓
  - Tools adaptation (Task 6) ✓
  - Main.rs integration (Task 8) ✓
  - Error handling (covered in Task 7 skip-corrupted-line test + Task 8 warnings) ✓
  - Tests (Task 7 unit tests + Task 8 integration) ✓

- [ ] **Placeholder scan:** No "TBD", "TODO", "implement later", or vague instructions remain.

- [ ] **Type consistency:**
  - `Part::ToolUse` fields: `id`, `name`, `arguments` used consistently across Tasks 2, 4, 5, 6, 7.
  - `Message::new` signature: `session_id: impl Into<String>, role: Role, parts: Vec<Part>` used in Tasks 2 and 8.
  - `SessionManager` methods names and signatures consistent between Task 7 definition and Task 8 usage.
