use clap::{
    builder::{
        styling::{AnsiColor, Effects},
        Styles,
    },
    Parser,
};

use super::{
    CatCommand, DecodeCommand, EncodeCommand, GrepCommand, InfoCommand, PipeCommand, QcCommand,
    RevcompCommand, SampleCommand, SplitCommand,
};

// Configures Clap v3-style help menu colors
const STYLES: Styles = Styles::styled()
    .header(AnsiColor::Green.on_default().effects(Effects::BOLD))
    .usage(AnsiColor::Green.on_default().effects(Effects::BOLD))
    .literal(AnsiColor::Cyan.on_default().effects(Effects::BOLD))
    .placeholder(AnsiColor::Yellow.on_default());

#[derive(Parser)]
#[command(styles = STYLES)]
#[clap(author, version, about, long_about = None)]
pub struct Cli {
    #[clap(subcommand)]
    pub command: Commands,
}

#[derive(Parser)]
pub enum Commands {
    Encode(EncodeCommand),

    Decode(DecodeCommand),

    Cat(CatCommand),

    Info(InfoCommand),

    Grep(GrepCommand),

    Sample(SampleCommand),

    Split(SplitCommand),

    Pipe(PipeCommand),

    Qc(QcCommand),

    Revcomp(RevcompCommand),
}
