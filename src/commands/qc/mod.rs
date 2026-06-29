use anyhow::Result;
use binseq::{BinseqReader, ParallelReader};

use crate::cli::{QcCommand, QcOptions};

mod base_quality;
mod proc;
mod seq_quality;

pub const PHRED_OFFSET: u8 = 33;
pub type QualAbundance = [usize; 94];
pub const DEFAULT_QUAL_ABUNDANCE: QualAbundance = [0; 94];

#[derive(Clone, Copy)]
pub struct QcConfig {
    per_base_qual: bool,
    per_seq_qual: bool,
}
impl QcConfig {
    fn from_opts(opts: &QcOptions) -> Self {
        Self {
            per_base_qual: !opts.skip_base_qual,
            per_seq_qual: !opts.skip_seq_qual,
        }
    }
}

pub fn run(args: &QcCommand) -> Result<()> {
    let reader = BinseqReader::new(args.input.path())?;
    let mut proc = proc::QcProcessor::new(&args.qc.outdir, QcConfig::from_opts(&args.qc));

    if let Some(mut span) = args.input.span {
        let range = span.get_range(reader.num_records()?)?;
        reader.process_parallel_range(proc.clone(), args.qc.threads, range)?;
    } else {
        reader.process_parallel(proc.clone(), args.qc.threads)?;
    }
    proc.finish()?;

    Ok(())
}
