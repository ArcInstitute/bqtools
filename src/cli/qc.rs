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
pub struct QcOptions {
    /// Number of threads to use [0: auto]
    #[clap(short = 'T', long, default_value_t = 0)]
    pub threads: usize,

    /// Skip per-base-quality
    #[clap(long)]
    pub skip_base_qual: bool,

    /// Skip per-seq-quality
    #[clap(long)]
    pub skip_seq_qual: bool,

    /// Skip per-base-content
    #[clap(long)]
    pub skip_base_content: bool,

    /// Path to output directory write to
    #[clap(short, long, default_value = "./bqtools-qc")]
    pub outdir: String,
}
