use anyhow::Result;
use binseq::{BinseqReader, ParallelReader};

use crate::cli::QcCommand;

mod base_quality;
mod config;
mod modules;
mod proc;
mod seq_quality;

use config::QcConfig;
use modules::QcModule;

pub const PHRED_OFFSET: u8 = 33;
pub type QualAbundance = [usize; 94];
pub const DEFAULT_QUAL_ABUNDANCE: QualAbundance = [0; 94];

pub fn run(args: &QcCommand) -> Result<()> {
    let reader = BinseqReader::new(args.input.path())?;
    let mut proc = proc::QcProcessor::new(&args.qc.outdir, QcConfig::from_opts(&args.qc))?;

    if let Some(mut span) = args.input.span {
        let range = span.get_range(reader.num_records()?)?;
        reader.process_parallel_range(proc.clone(), args.qc.threads, range)?;
    } else {
        reader.process_parallel(proc.clone(), args.qc.threads)?;
    }
    proc.finish()?;

    Ok(())
}
