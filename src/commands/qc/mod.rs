use std::path::Path;

use anyhow::Result;
use binseq::{BinseqReader, ParallelReader};

use crate::{
    cli::QcCommand,
    commands::{match_output, utils::make_directory},
    types::BoxedWriter,
};

mod base_quality;
mod proc;

use base_quality::PerBaseSequenceQuality;
use proc::QcProcessor;

fn open_bsq<P: AsRef<Path>>(
    outdir: &P,
    paired: bool,
) -> Result<(BoxedWriter, Option<BoxedWriter>)> {
    let open_bsq = |primary: bool| -> Result<BoxedWriter> {
        if primary {
            match_output(Some(outdir.as_ref().join("bsq_r1.tsv")))
        } else {
            match_output(Some(outdir.as_ref().join("bsq_r2.tsv")))
        }
    };

    if !outdir.as_ref().exists() {
        make_directory(outdir)?;
    }

    if paired {
        Ok((open_bsq(true)?, Some(open_bsq(false)?)))
    } else {
        Ok((open_bsq(true)?, None))
    }
}

pub fn run(args: &QcCommand) -> Result<()> {
    let reader = BinseqReader::new(args.input.path())?;
    let proc = QcProcessor::default();

    // open writer handles
    let (mut s_handle_bsq, mut x_handle_bsq) = open_bsq(&args.qc.outdir, reader.is_paired())?;

    if let Some(mut span) = args.input.span {
        let range = span.get_range(reader.num_records()?)?;
        reader.process_parallel_range(proc.clone(), args.qc.threads, range)?;
    } else {
        reader.process_parallel(proc.clone(), args.qc.threads)?;
    }

    proc.bsq.pprint(&mut s_handle_bsq, x_handle_bsq.as_mut())?;

    Ok(())
}
