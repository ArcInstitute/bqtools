use anyhow::Result;
use clap::Parser;

use crate::cli::FileFormat;

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
impl GrepCommand {
    pub fn should_color(&self) -> bool {
        match self.output.format() {
            Ok(FileFormat::Bam) => false,
            _ => {
                self.output.output.is_none()
                    && self.output.prefix.is_none()
                    && self.grep.color.should_color()
            }
        }
    }
}

#[derive(Parser, Debug)]
#[clap(next_help_heading = "SEARCH OPTIONS")]
pub struct GrepArgs {
    /// Regex expression to search for in primary sequence
    #[clap(short = 'r', long)]
    pub reg1: Vec<String>,

    /// Regex expression to search for in extended sequence
    #[clap(short = 'R', long)]
    pub reg2: Vec<String>,

    /// Regex expression to search for in either sequence
    pub reg: Vec<String>,

    /// Invert pattern criteria (like grep -v)
    #[clap(short = 'v', long)]
    pub invert: bool,

    /// Only count matches
    #[clap(short = 'C', long)]
    pub count: bool,

    /// use OR logic for multiple patterns (default=AND)
    #[clap(long)]
    pub or_logic: bool,

    /// Fuzzy finding using `sassy`
    ///
    /// Note that regex expressions are not supported with this flag.
    #[clap(short = 'z', long)]
    pub fuzzy: bool,

    /// Maximum edit distance to allow when fuzzy matching
    ///
    /// Only used with fuzzy matching
    #[clap(short = 'k', long, default_value = "1")]
    pub distance: usize,

    /// Colorize output (auto, always, never)
    #[clap(
        long,
        value_name = "WHEN",
        default_value = "auto",
        conflicts_with = "format"
    )]
    color: ColorWhen,
}

impl GrepArgs {
    pub fn validate(&self) -> Result<()> {
        if self.reg1.is_empty() && self.reg2.is_empty() && self.reg.is_empty() {
            anyhow::bail!("At least one pattern must be specified");
        }
        Ok(())
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
    pub fn and_logic(&self) -> bool {
        !self.or_logic
    }
}

#[cfg(feature = "fuzzy")]
impl GrepArgs {
    pub fn bytes_pat1(&self) -> Vec<Vec<u8>> {
        self.reg1.iter().map(|s| s.as_bytes().to_vec()).collect()
    }
    pub fn bytes_pat2(&self) -> Vec<Vec<u8>> {
        self.reg2.iter().map(|s| s.as_bytes().to_vec()).collect()
    }
    pub fn bytes_pat(&self) -> Vec<Vec<u8>> {
        self.reg.iter().map(|s| s.as_bytes().to_vec()).collect()
    }
}

#[derive(Clone, Debug, clap::ValueEnum)]
pub enum ColorWhen {
    Auto,
    Always,
    Never,
}

impl ColorWhen {
    pub fn should_color(&self) -> bool {
        match self {
            ColorWhen::Always => true,
            ColorWhen::Never => false,
            ColorWhen::Auto => {
                use is_terminal::IsTerminal;
                std::io::stdout().is_terminal()
            }
        }
    }
}
