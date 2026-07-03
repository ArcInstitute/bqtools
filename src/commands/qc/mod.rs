use anyhow::Result;
use binseq::{BinseqReader, ParallelReader};
use log::trace;

use crate::cli::QcCommand;

mod base_content;
mod base_quality;
mod config;
mod dup_levels;
mod gc_content;
mod modules;
mod proc;
mod report;
mod seq_length;
mod seq_quality;

use config::QcConfig;
use modules::QcModule;

pub const PHRED_OFFSET: u8 = 33;
pub type QualAbundance = [usize; 94];
pub const DEFAULT_QUAL_ABUNDANCE: QualAbundance = [0; 94];

pub fn run(args: &QcCommand) -> Result<()> {
    let reader = BinseqReader::new(args.input.path())?;
    let paired = reader.is_paired();
    let total_records = reader.num_records()?;
    let range = args
        .input
        .span
        .map(|mut span| span.get_range(total_records))
        .transpose()?;
    let processed_records = range.as_ref().map_or(total_records, |r| r.end - r.start);

    let mut proc = proc::QcProcessor::new(
        &args.qc.outdir,
        QcConfig::from_opts(&args.qc),
        args.input.path().to_string(),
        processed_records,
        paired,
    )?;

    if let Some(range) = range {
        trace!("Processing span: {}..{}", range.start, range.end);
        reader.process_parallel_range(proc.clone(), args.qc.threads, range)?;
    } else {
        trace!("Processing all records: n={total_records}");
        reader.process_parallel(proc.clone(), args.qc.threads)?;
    }
    proc.finish()?;

    Ok(())
}
