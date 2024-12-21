use clap::Parser;

use super::{InputFastq, OutputBinseq};

#[derive(Parser, Debug)]
pub enum ImportCommand {
    Fastq(FastqImport),
}

#[derive(Parser, Debug)]
pub struct FastqImport {
    #[clap(flatten)]
    pub input: InputFastq,

    #[clap(flatten)]
    pub output: OutputBinseq,
}
