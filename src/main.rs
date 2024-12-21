mod cli;
mod commands;

use cli::{Cli, Commands, ExportCommand, ImportCommand};

use anyhow::Result;
use clap::Parser;

fn main() -> Result<()> {
    let args = Cli::parse();
    match args.command {
        Commands::Import(import) => match import {
            ImportCommand::Fastq(args) => commands::import::fastq::run(args),
        },
        Commands::Export(export) => match export {
            ExportCommand::Fastq(args) => commands::export::fastq::run(args),
        },
    }
}
