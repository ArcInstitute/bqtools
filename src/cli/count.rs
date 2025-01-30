use clap::Parser;

use super::InputBinseq;

#[derive(Parser, Debug)]
/// Count the number of records in a BINSEQ file.
pub struct CountCommand {
    #[clap(flatten)]
    pub input: InputBinseq,

    /// Skips the integrity check of the BINSEQ file if possible.
    ///
    /// This is only possible for non-compressed BINSEQ files.
    #[clap(short = 's', long)]
    pub skip_val: bool,
}
