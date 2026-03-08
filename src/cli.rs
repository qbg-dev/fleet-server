use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "boring-mail", about = "A Gmail-conformant mail server for AI agents")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Start the mail server (default)
    Serve,
    /// Initialize data directory and database
    Init,
    /// Show server and database status
    Status,
    /// List registered accounts
    Accounts,
}
