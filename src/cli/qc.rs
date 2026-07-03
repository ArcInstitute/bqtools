use clap::Parser;

use super::InputBinseq;

#[derive(Parser, Debug)]
pub struct QcCommand {
    #[clap(flatten)]
    pub input: InputBinseq,

    #[clap(flatten)]
    pub qc: QcOptions,
}

// Each `skip_*` flag independently toggles one QC module on/off - they're
// orthogonal CLI switches, not states in a state machine, so collapsing them
// into an enum wouldn't fit clap's flag model or improve readability here.
#[allow(clippy::struct_excessive_bools)]
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

    /// Skip per-sequence-gc-content
    #[clap(long)]
    pub skip_seq_gc: bool,

    /// Skip sequence-length-distribution
    #[clap(long)]
    pub skip_seq_length: bool,

    /// Skip sequence-duplication-levels
    #[clap(long)]
    pub skip_dup_levels: bool,

    /// Skip overrepresented-sequences
    #[clap(long)]
    pub skip_overrepresented: bool,

    /// Number of leading records (by file order) to sample for duplication
    /// level and overrepresented-sequence estimation [0: use all records]
    #[clap(long, default_value_t = 100_000)]
    pub dup_sample_size: usize,

    /// Minimum percentage of sampled reads a sequence must represent to be
    /// flagged as overrepresented
    #[clap(long, default_value_t = 0.1)]
    pub overrepresented_threshold: f64,

    /// Path to output directory write to
    #[clap(short, long, default_value = "./bqtools-qc")]
    pub outdir: String,
}
