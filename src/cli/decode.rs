use clap::Parser;

use super::{InputBinseq, OutputFile};

/// Decode BINSEQ files to FASTQ or FASTA.
#[derive(Parser, Debug)]
pub struct DecodeCommand {
    #[clap(flatten)]
    pub input: InputBinseq,

    #[clap(flatten)]
    pub output: OutputFile,
}
