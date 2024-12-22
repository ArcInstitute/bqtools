use clap::Parser;

use super::{InputFasta, InputFastq, OutputBinseq};

#[derive(Parser, Debug)]
pub enum ImportCommand {
    Fastq(FastqImport),
    Fasta(FastaImport),
}

#[derive(Parser, Debug)]
pub struct FastqImport {
    #[clap(flatten)]
    pub input: InputFastq,

    #[clap(flatten)]
    pub output: OutputBinseq,
}

#[derive(Parser, Debug)]
pub struct FastaImport {
    #[clap(flatten)]
    pub input: InputFasta,

    #[clap(flatten)]
    pub output: OutputBinseq,
}
