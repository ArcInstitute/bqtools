use clap::Parser;

use super::{InputBinseq, OutputFile};

#[derive(Parser, Debug)]
pub struct DecodeCommand {
    #[clap(flatten)]
    pub input: InputBinseq,

    #[clap(flatten)]
    pub output: OutputFile,
}
