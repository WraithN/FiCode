#![allow(warnings)]

#![allow(warnings)]

// =============================================================================
// Rust 基础概念：模块系统
// =============================================================================
// `mod` 关键字声明当前 crate 包含的模块，Rust 编译器会在对应目录查找

mod agent;
mod permission;
mod provider;
mod tools;

// `anyhow` 是一个错误处理库，提供了简化错误传播的功能
use anyhow::Result;

// `colored` 库用于终端彩色输出
use colored::Colorize;

// `rustyline` 是一个命令行读取库（类似 GNU readline）
use rustyline::DefaultEditor;

// `json!` 是 serde_json 提供的宏，用于方便地创建 JSON 值
use serde_json::json;

use agent::{agent_loop, ContentBlock, LoopState, Message};
use provider::{Model, Provider};

// =============================================================================
// 程序入口：main 函数
// =============================================================================

// `#[tokio::main]` 是属性宏，将 main 函数包装在 tokio 异步运行时中
#[tokio::main]
async fn main() -> Result<()> {
    let model = Model::get_model()?;
    let mut provider = Provider::new();
    provider.set_model(model);
    let client = provider.get_client()?;
    let mut editor = DefaultEditor::new()?;
    let mut history: Vec<Message> = Vec::new();

    loop {
        let readline = editor.readline("s01 >> ".cyan().to_string().as_str());

        match readline {
            Ok(line) => {
                let query = line.trim();

                if query.is_empty() || ["q", "exit"].contains(&query.to_lowercase().as_str()) {
                    break;
                }

                editor.add_history_entry(query)?;

                history.push(Message {
                    role: "user".to_string(),
                    content: Some(json!(query)),
                });

                let mut state = LoopState::new(history.clone());

                agent_loop(client.as_ref(), &mut state).await?;

                history = state.messages;

                if let Some(last_msg) = history.last() {
                    if last_msg.role == "assistant" {
                        if let Some(content) = &last_msg.content {
                            if let Ok(blocks) =
                                serde_json::from_value::<Vec<ContentBlock>>(content.clone())
                            {
                                let text = provider::extract_text(&blocks);
                                if !text.is_empty() {
                                    println!("{}", text);
                                }
                            }
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
