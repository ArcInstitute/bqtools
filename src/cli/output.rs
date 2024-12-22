use anyhow::Result;
use clap::Parser;
use std::io::Write;

use crate::commands::match_output;

#[derive(Parser, Debug)]
pub struct OutputFastq {
    #[clap(short = 'o', long, help = "Output FASTQ file [default: stdout]")]
    pub output: Option<String>,
}
impl OutputFastq {
    pub fn as_writer(&self) -> Result<Box<dyn Write>> {
        match_output(self.output.as_ref())
    }
}

#[derive(Parser, Debug)]
pub struct OutputFasta {
    #[clap(short = 'o', long, help = "Output FASTA file [default: stdout]")]
    pub output: Option<String>,
}
impl OutputFasta {
    pub fn as_writer(&self) -> Result<Box<dyn Write>> {
        match_output(self.output.as_ref())
    }
}

#[derive(Parser, Debug)]
pub struct OutputBinseq {
    #[clap(short = 'o', long, help = "Output binseq file [default: stdout]")]
    pub output: Option<String>,
}
impl OutputBinseq {
    pub fn as_writer(&self) -> Result<Box<dyn Write>> {
        match_output(self.output.as_ref())
    }
}
