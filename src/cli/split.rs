use clap::Parser;

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

    /// Don't use Aho-Corasick DFA (slower, but lower memory)
    #[clap(long)]
    pub no_dfa: bool,

    /// Number of processing threads to use, 0: auto
    #[clap(short = 'T', long, default_value_t = 0)]
    pub threads: usize,
}
