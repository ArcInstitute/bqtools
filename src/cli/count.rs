use clap::Parser;

use super::InputBinseq;

#[derive(Parser, Debug)]
/// Count the number of records in a BINSEQ file.
pub struct CountCommand {
    #[clap(flatten)]
    pub input: InputBinseq,

    #[clap(flatten)]
    pub opts: CountOpts,
}

#[derive(Parser, Debug)]
#[clap(next_help_heading = "COUNT OPTIONS")]
pub struct CountOpts {
    /// Only print the number of records in the file
    #[clap(short, long)]
    pub num: bool,
}
