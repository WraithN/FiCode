use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;

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
    fields.insert("status".to_string(), serde_json::to_value(&session.status).unwrap());
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{Part, Role};
    use std::io::Write;

    fn temp_manager() -> (SessionManager, tempfile::TempDir) {
        let dir = tempfile::TempDir::new().unwrap();
        let manager = SessionManager::new(dir.path().to_path_buf());
        (manager, dir)
    }

    #[test]
    fn test_create_and_load_session() {
        let (manager, _dir) = temp_manager();
        let session = manager.create_session("claude-test").unwrap();
        assert_eq!(session.model, "claude-test");
        assert!(session.messages.is_empty());

        let loaded = manager.load_session(&session.id).unwrap();
        assert_eq!(loaded.id, session.id);
        assert_eq!(loaded.model, session.model);
    }

    #[test]
    fn test_append_and_load_message() {
        let (manager, _dir) = temp_manager();
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
        let (manager, dir) = temp_manager();
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
