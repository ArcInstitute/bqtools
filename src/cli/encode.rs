use clap::Parser;

use super::{InputFile, OutputBinseq};

#[derive(Parser, Debug)]
pub struct EncodeCommand {
    #[clap(flatten)]
    pub input: InputFile,

    #[clap(flatten)]
    pub output: OutputBinseq,
}
