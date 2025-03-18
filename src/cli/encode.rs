use clap::Parser;

use super::{InputFile, OutputBinseq};

#[derive(Parser, Debug, Clone)]
/// Encode FASTQ or FASTA files to BINSEQ.
pub struct EncodeCommand {
    #[clap(flatten)]
    pub input: InputFile,

    #[clap(flatten)]
    pub output: OutputBinseq,
}
