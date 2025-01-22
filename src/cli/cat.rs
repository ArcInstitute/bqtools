use clap::Parser;

use super::{MultiInputBinseq, OutputBinseq};

#[derive(Parser, Debug)]
/// Concatenate BINSEQ files.
pub struct CatCommand {
    #[clap(flatten)]
    pub input: MultiInputBinseq,

    #[clap(flatten)]
    pub output: OutputBinseq,
}
