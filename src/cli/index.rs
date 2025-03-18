use clap::Parser;

use super::InputBinseq;

/// Index the blocks of a VBQ
#[derive(Parser, Debug)]
pub struct IndexCommand {
    #[clap(flatten)]
    pub input: InputBinseq,

    /// Print statistics about the index
    #[clap(short, long)]
    pub verbose: bool,
}
