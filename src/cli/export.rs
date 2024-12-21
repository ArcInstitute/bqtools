use clap::Parser;

use super::{InputBinseq, OutputFastq};

#[derive(Parser, Debug)]
pub enum ExportCommand {
    Fastq(FastqExport),
}

#[derive(Parser, Debug)]
pub struct FastqExport {
    #[clap(flatten)]
    pub input: InputBinseq,

    #[clap(flatten)]
    pub output: OutputFastq,
}
