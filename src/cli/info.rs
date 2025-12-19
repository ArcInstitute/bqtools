use clap::Parser;

use super::InputBinseq;

#[derive(Parser, Debug)]
/// Count the number of records in a BINSEQ file.
pub struct InfoCommand {
    #[clap(flatten)]
    pub input: InputBinseq,

    #[clap(flatten)]
    pub opts: InfoOpts,
}

#[derive(Parser, Debug)]
#[clap(next_help_heading = "INFO OPTIONS")]
pub struct InfoOpts {
    /// Only print the number of records in the file
    #[clap(short, long)]
    pub num: bool,

    /// Print the index of the file
    #[clap(long)]
    pub show_index: bool,

    /// Print the block headers of the file
    #[clap(long, conflicts_with = "show_index")]
    pub show_headers: bool,
}
