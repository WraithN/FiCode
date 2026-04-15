#![allow(warnings)]

// =============================================================================
// Rust 基础概念：模块系统
// =============================================================================
// `mod` 关键字声明当前 crate 包含的模块，Rust 编译器会在对应目录查找

mod agent;
mod permission;
mod provider;
mod session;
mod tools;

// `anyhow` 是一个错误处理库，提供了简化错误传播的功能
use anyhow::Result;

// `colored` 库用于终端彩色输出
use colored::Colorize;

// `rustyline` 是一个命令行读取库（类似 GNU readline）
use rustyline::DefaultEditor;

use agent::{agent_loop, LoopState, Message, Role};
use provider::{Model, Provider};
use session::{SessionManager, SessionMeta, SessionStatus};
use std::path::PathBuf;

// =============================================================================
// 程序入口：main 函数
// =============================================================================

// `#[tokio::main]` 是属性宏，将 main 函数包装在 tokio 异步运行时中
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
    let prompt_prefix = format!("{} >> ", &session.id[..session.id.len().min(8)]);

    loop {
        let readline = editor.readline(prompt_prefix.cyan().to_string().as_str());

        match readline {
            Ok(line) => {
                let query = line.trim();

                if query.is_empty() || ["q", "exit"].contains(&query.to_lowercase().as_str()) {
                    break;
                }

                editor.add_history_entry(query)?;

                let user_msg = Message::new(
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

                // Persist session after each turn
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
            &s.id[..s.id.len().min(8)],
            s.project_path,
            s.message_count,
            if s.status == SessionStatus::Active {
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
