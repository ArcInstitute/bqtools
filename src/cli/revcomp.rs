use clap::Parser;

use super::{InputBinseq, Mate, OutputBinseq};

/// Reverse complement the sequences in a BINSEQ file.
#[derive(Parser, Debug)]
pub struct RevcompCommand {
    #[clap(flatten)]
    pub input: InputBinseq,

    #[clap(flatten)]
    pub output: OutputBinseq,

    /// Which mate(s) to reverse complement
    ///
    /// Only relevant for paired BINSEQ files. Defaults to reverse
    /// complementing both mates; ignored (with a warning) on single-end
    /// files.
    ///
    /// Note: `-m` is already used by `--mode` (BINSEQ output format), so
    /// this flag uses `-M` instead.
    #[clap(short = 'M', long, default_value = "both")]
    pub mate: Mate,
}
