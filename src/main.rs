#![allow(clippy::module_inception)]

mod cli;
mod commands;

use anyhow::Result;
use clap::Parser;

use cli::{Cli, Commands};

#[cfg(unix)]
fn reset_sigpipe() {
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
}

#[cfg(not(unix))]
fn reset_sigpipe() {
    // no-op
}

fn main() -> Result<()> {
    // Handle Ctrl+C gracefully
    reset_sigpipe();

    let args = Cli::parse();
    match args.command {
        Commands::Encode(encode) => commands::encode::run(encode),
        Commands::Decode(decode) => commands::decode::run(decode),
        Commands::Cat(cat) => commands::cat::run(cat),
        Commands::Count(count) => commands::count::run(count),
        Commands::Index(index) => commands::index::run(index),
        Commands::Grep(grep) => commands::grep::run(grep),
    }
}
