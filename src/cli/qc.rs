use clap::Parser;

use super::InputBinseq;

#[derive(Parser, Debug)]
pub struct QcCommand {
    #[clap(flatten)]
    pub input: InputBinseq,

    #[clap(flatten)]
    pub qc: QcOptions,
}

#[derive(Parser, Debug)]
#[clap(next_help_heading = "QC OPTIONS")]
pub struct QcOptions {}
