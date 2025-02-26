use clap::Parser;

use super::InputBinseq;

#[derive(Parser, Debug)]
/// Count the number of records in a BINSEQ file.
pub struct CountCommand {
    #[clap(flatten)]
    pub input: InputBinseq,
}
