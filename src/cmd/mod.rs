use clap::{arg, command, Parser};


#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct CmdArgs {
    /// Source Path to copy
    #[arg(short, long)]
    pub source: String,

    /// Destination path
    #[arg(short, long)]
    pub destination: String,

    /// SSH connection string in the format user@host
    #[arg(long)]
    pub ssh: String,
}
