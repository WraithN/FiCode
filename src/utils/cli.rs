use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "fi-code", version = env!("CARGO_PKG_VERSION"))]
pub struct Args {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Enable debug logging (debug|trace|info|off, default: info)
    #[cfg(debug_assertions)]
    #[arg(
        short = 'l',
        long = "log",
        value_name = "LEVEL",
        default_value = "info"
    )]
    pub log_level: String,

    /// Enter interactive REPL mode
    #[arg(short = 'i', long = "interactive")]
    pub interactive: bool,

    /// Print session information and exit
    #[arg(short = 's', long = "session", value_name = "SESSION", num_args = 0..=1)]
    pub session: Option<Option<String>>,

    /// Execute a single command and exit
    #[arg(short = 'c', long = "command", value_name = "MESSAGE")]
    pub cmd: Option<String>,

    /// Show configured providers and models
    #[arg(short = 'm', long = "models")]
    pub models: bool,

    /// Workspace directory (default: home directory)
    #[arg(short = 'w', long = "workspace", value_name = "PATH")]
    pub workspace: Option<PathBuf>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Start the web server
    Server {
        /// Port to listen on
        #[arg(short, long)]
        port: Option<u16>,
    },
}
