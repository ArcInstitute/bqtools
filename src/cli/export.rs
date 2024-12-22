use clap::Parser;

use super::{InputBinseq, OutputFasta, OutputFastq};

#[derive(Parser, Debug)]
pub enum ExportCommand {
    Fastq(FastqExport),
    Fasta(FastaExport),
}

#[derive(Parser, Debug)]
pub struct FastqExport {
    #[clap(flatten)]
    pub input: InputBinseq,

    #[clap(flatten)]
    pub output: OutputFastq,
}

#[derive(Parser, Debug)]
pub struct FastaExport {
    #[clap(flatten)]
    pub input: InputBinseq,

    #[clap(flatten)]
    pub output: OutputFasta,
}
