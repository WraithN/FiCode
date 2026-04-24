#![allow(warnings)]

mod agent;
mod commands;
mod config;
mod entry;
mod mcp;
mod permission;
mod provider;
mod session;
mod skills;
mod task;
mod tools;
mod utils;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    entry::run().await
}
