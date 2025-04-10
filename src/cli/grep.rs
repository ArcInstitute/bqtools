use anyhow::Result;
use clap::Parser;

use super::{InputBinseq, OutputFile};

/// Grep a BINSEQ file and output to FASTQ or FASTA.
#[derive(Parser, Debug)]
pub struct GrepCommand {
    #[clap(flatten)]
    pub input: InputBinseq,

    #[clap(flatten)]
    pub output: OutputFile,

    #[clap(flatten)]
    pub grep: GrepArgs,
}

#[derive(Parser, Debug)]
#[clap(next_help_heading = "SEARCH OPTIONS")]
pub struct GrepArgs {
    /// Pattern to search for in mate 1
    #[clap(short = 'e', long)]
    pub pat1: Vec<String>,
    /// Pattern to search for in mate 2
    #[clap(short = 'E', long)]
    pub pat2: Vec<String>,
    /// Pattern to search for in both mates
    #[clap(short = 'P', long)]
    pub pat: Vec<String>,
    /// Invert pattern criteria (like grep -v)
    #[clap(short = 'v', long)]
    pub invert: bool,
}
impl GrepArgs {
    pub fn validate(&self) -> Result<()> {
        if self.pat1.is_empty() && self.pat2.is_empty() && self.pat.is_empty() {
            anyhow::bail!("At least one pattern must be specified");
        }
        Ok(())
    }

    pub fn bytes_mp1(&self) -> Vec<Vec<u8>> {
        self.pat1.iter().map(|s| s.as_bytes().to_vec()).collect()
    }
    pub fn bytes_mp2(&self) -> Vec<Vec<u8>> {
        self.pat2.iter().map(|s| s.as_bytes().to_vec()).collect()
    }
    pub fn bytes_pat(&self) -> Vec<Vec<u8>> {
        self.pat.iter().map(|s| s.as_bytes().to_vec()).collect()
    }
}
