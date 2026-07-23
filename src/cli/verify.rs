use clap::Parser;

use super::{InputBinseq, Mate};

/// Compute an order-independent checksum over a BINSEQ file.
///
/// BINSEQ files are frequently produced by parallel encoders, which make no
/// guarantee that record order matches the input FASTQ/FASTA. `verify`
/// accounts for this by hashing each record independently and combining the
/// per-record hashes with a commutative operation (wrapping sum), so the
/// resulting checksum is identical regardless of record order.
///
/// Use this to confirm that two BINSEQ files (or two encode runs of the same
/// input) carry the same data even if a parallel encoder wrote them in
/// different record orders.
#[derive(Parser, Debug)]
pub struct VerifyCommand {
    #[clap(flatten)]
    pub input: InputBinseq,

    #[clap(flatten)]
    pub opts: VerifyOptions,
}

// Each `skip_*` flag independently toggles one field out of the checksum -
// they're orthogonal CLI switches, not states in a state machine, so
// collapsing them into an enum wouldn't fit clap's flag model here.
#[allow(clippy::struct_excessive_bools)]
#[derive(Parser, Debug)]
#[clap(next_help_heading = "VERIFY OPTIONS")]
pub struct VerifyOptions {
    /// Exclude sequence data from the checksum
    #[clap(long)]
    pub skip_seq: bool,

    /// Exclude quality scores from the checksum
    #[clap(long)]
    pub skip_qual: bool,

    /// Exclude sequence/record headers from the checksum
    #[clap(long)]
    pub skip_headers: bool,

    /// Exclude the per-record flag from the checksum
    #[clap(long)]
    pub skip_flags: bool,

    /// Which mate(s) to include in the checksum for paired records
    ///
    /// Ignored (with a warning) on single-end files.
    ///
    /// Note: `-m` is already used by `--mode` on other commands, so this
    /// flag uses `-M` for consistency with `revcomp`.
    #[clap(short = 'M', long, default_value = "both")]
    pub mate: Mate,

    /// Number of threads to use [0: auto]
    #[clap(short = 'T', long, default_value_t = 0)]
    pub threads: usize,

    /// Print the checksum report as JSON
    #[clap(short, long)]
    pub json: bool,
}
