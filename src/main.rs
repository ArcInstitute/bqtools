#![allow(clippy::module_inception)]

mod cli;
mod commands;

use anyhow::Result;
use clap::Parser;

use cli::{Cli, Commands};

fn main() -> Result<()> {
    let args = Cli::parse();
    match args.command {
        Commands::Encode(encode) => commands::encode::run(encode),
        Commands::Decode(decode) => commands::decode::run(decode),
    }
}
