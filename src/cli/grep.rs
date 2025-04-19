use anyhow::Result;
use clap::Parser;
use memchr::memmem::Finder;

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
    /// Fixed string pattern to search for in primary sequence
    #[clap(short = 'e', long)]
    pub pat1: Vec<String>,

    /// Fixed string pattern to search for in extended sequence
    #[clap(short = 'E', long)]
    pub pat2: Vec<String>,

    /// Pattern to search for in either sequence
    #[clap(short = 'F', long)]
    pub pat: Vec<String>,

    /// Regex expression to search for in primary sequence
    #[clap(short = 'r', long)]
    pub reg1: Vec<String>,

    /// Regex expression to search for in extended sequence
    #[clap(short = 'R', long)]
    pub reg2: Vec<String>,

    /// Regex expression to search for in either sequence
    #[clap(short = 'P', long)]
    pub reg: Vec<String>,

    /// Invert pattern criteria (like grep -v)
    #[clap(short = 'v', long)]
    pub invert: bool,

    /// Only count matches
    #[clap(short = 'C', long)]
    pub count: bool,
}
impl GrepArgs {
    pub fn validate(&self) -> Result<()> {
        if self.pat1.is_empty()
            && self.pat2.is_empty()
            && self.pat.is_empty()
            && self.reg1.is_empty()
            && self.reg2.is_empty()
            && self.reg.is_empty()
        {
            anyhow::bail!("At least one pattern must be specified");
        }
        Ok(())
    }
    pub fn bytes_mp1(&self) -> Vec<Finder<'static>> {
        self.pat1
            .iter()
            .map(|s| Finder::new(s.as_bytes()))
            .map(|f| f.into_owned())
            .collect()
    }
    pub fn bytes_mp2(&self) -> Vec<Finder<'static>> {
        self.pat2
            .iter()
            .map(|s| Finder::new(s.as_bytes()))
            .map(|f| f.into_owned())
            .collect()
    }
    pub fn bytes_pat(&self) -> Vec<Finder<'static>> {
        self.pat
            .iter()
            .map(|s| Finder::new(s.as_bytes()))
            .map(|f| f.into_owned())
            .collect()
    }
    pub fn bytes_reg1(&self) -> Vec<regex::bytes::Regex> {
        self.reg1
            .iter()
            .map(|s| regex::bytes::Regex::new(s).expect("Could not build regex from pattern: {s}"))
            .collect()
    }
    pub fn bytes_reg2(&self) -> Vec<regex::bytes::Regex> {
        self.reg2
            .iter()
            .map(|s| regex::bytes::Regex::new(s).expect("Could not build regex from pattern: {s}"))
            .collect()
    }
    pub fn bytes_reg(&self) -> Vec<regex::bytes::Regex> {
        self.reg
            .iter()
            .map(|s| regex::bytes::Regex::new(s).expect("Could not build regex from pattern: {s}"))
            .collect()
    }
}
