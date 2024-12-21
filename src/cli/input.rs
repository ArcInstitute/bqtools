use anyhow::Result;
use clap::Parser;
use std::io::Read;

use crate::commands::match_input;

#[derive(Parser, Debug)]
pub struct InputFastq {
    #[clap(short = 'i', long, help = "Input FASTQ file [default: stdin]")]
    pub input: Option<String>,
}
impl InputFastq {
    pub fn as_reader(&self) -> Result<Box<dyn Read>> {
        match_input(self.input.as_ref())
    }
}

#[derive(Parser, Debug)]
pub struct InputBinseq {
    #[clap(short = 'i', long, help = "Input binseq file [default: stdin]")]
    pub input: Option<String>,
}
impl InputBinseq {
    pub fn as_reader(&self) -> Result<Box<dyn Read>> {
        match_input(self.input.as_ref())
    }
}
