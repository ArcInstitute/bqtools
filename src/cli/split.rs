use clap::Parser;

use super::{InputBinseq, OutputBinseqOptions, PatternFileArgs};

/// Split a BINSEQ file into multiple files based on patterns provided in a pattern file
#[derive(Parser, Debug)]
pub struct SplitCommand {
    #[clap(flatten)]
    pub input: InputBinseq,

    #[clap(flatten)]
    pub split: SplitOptions,

    #[clap(flatten)]
    pub output: OutputBinseqOptions,

    #[clap(flatten)]
    pub patterns: PatternFileArgs,
}

#[derive(Parser, Debug)]
pub struct SplitOptions {
    /// Optional base path for output files. If not provided, the current working directory will be used.
    #[clap(long, default_value = ".")]
    pub basepath: String,

    /// Skip writing records that do not match any pattern.
    #[clap(long)]
    pub skip_unmatched: bool,

    /// Optional base name for unmatched records.
    #[clap(long, default_value = "unmatched")]
    pub unmatched_basename: String,
}
