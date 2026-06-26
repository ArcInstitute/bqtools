use anyhow::Result;
use clap::Parser;

#[cfg(feature = "fuzzy")]
use super::FuzzyArgs;
use super::{InputBinseq, PatternFileArgs};

/// Split a BINSEQ file into multiple files based on patterns provided in a pattern file
#[derive(Parser, Debug)]
pub struct SplitCommand {
    #[clap(flatten)]
    pub input: InputBinseq,

    #[clap(flatten)]
    pub split: SplitOptions,

    #[clap(flatten)]
    pub patterns: PatternFileArgs,

    #[cfg(feature = "fuzzy")]
    #[clap(flatten)]
    pub fuzzy_args: FuzzyArgs,
}

impl SplitCommand {
    pub fn validate(&self) -> Result<()> {
        if self.patterns.empty() {
            anyhow::bail!("At least one pattern file must be specified");
        }
        Ok(())
    }
}

#[derive(Parser, Debug)]
#[clap(next_help_heading = "SPLIT OPTIONS")]
pub struct SplitOptions {
    /// Optional base path for output files. If not provided, the current working directory will be used.
    #[clap(long, default_value = "./split_outs")]
    pub basepath: String,

    /// Skip writing records that do not match any pattern.
    #[clap(long)]
    pub skip_unmatched: bool,

    /// Optional base name for unmatched records.
    #[clap(long, default_value = "unmatched")]
    pub unmatched_basename: String,

    /// Remove output files with fewer than this many records.
    ///
    /// Defaults to 1, which removes empty output files. Set to 0 to keep all files.
    #[clap(long, default_value_t = 1)]
    pub min_records: usize,

    /// Denotes patterns are fixed strings (non-regex)
    ///
    /// Allows usage of Aho-Corasick algorithm for efficient matching.
    /// This is auto-detected when all patterns are literal strings.
    #[clap(short = 'x', long)]
    pub fixed: bool,

    /// Don't use Aho-Corasick DFA (slower, but lower memory)
    #[clap(long)]
    pub no_dfa: bool,

    /// Number of processing threads to use, 0: auto
    #[clap(short = 'T', long, default_value_t = 0)]
    pub threads: usize,

    /// Suppress the per-pattern record count summary written to stderr.
    #[clap(long)]
    pub quiet: bool,
}
