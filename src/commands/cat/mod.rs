use std::{fs::File, io::Write};

use anyhow::{bail, Result};
use binseq::{
    bq::{BinseqHeader, MmapReader, SIZE_HEADER},
    vbq::{self, VBinseqHeader},
    BinseqReader, ParallelReader,
};
use log::{error, trace};
use memmap2::MmapOptions;

use crate::{cli::CatCommand, commands::encode::processor::VBinseqProcessor};

fn strip_header(path: &str) -> Result<BinseqHeader> {
    let reader = MmapReader::new(path)?;
    Ok(reader.header())
}

fn recover_header(paths: &[String]) -> Result<BinseqHeader> {
    let mut exp_header = None;
    for path in paths {
        let header = strip_header(path)?;
        if let Some(exp) = exp_header {
            if exp != header {
                bail!("Inconsistent headers.");
            }
        } else {
            exp_header = Some(header);
        }
    }
    exp_header.ok_or_else(|| anyhow::anyhow!("No input files."))
}

fn is_all_bq(paths: &[String]) -> Result<bool> {
    let mut all_bq = true;
    let mut all_vbq = true;
    for path in paths {
        let reader = BinseqReader::new(path)?;
        match reader {
            BinseqReader::Bq(_) => all_vbq = false,
            BinseqReader::Vbq(_) => all_bq = false,
        }
    }
    match (all_bq, all_vbq) {
        (true, true) => bail!("No input files."),
        (true, false) => {
            trace!("All BQ files");
            Ok(true)
        } // all bq files
        (false, true) => {
            trace!("All VBQ files");
            Ok(false)
        } // all vbq files
        (false, false) => {
            error!("Inconsistent file types. Must provide either all BQ or all VBQ files.");
            bail!("Inconsistent file types.")
        }
    }
}

fn run_bq(args: CatCommand) -> Result<()> {
    let header = recover_header(&args.input.input)?;
    let mut out_handle = args.output.as_writer()?;

    header.write_bytes(&mut out_handle)?;
    for path in args.input.input {
        let file = File::open(path)?;
        let mmap = unsafe { MmapOptions::new().map(&file)? };
        out_handle.write_all(&mmap[SIZE_HEADER..])?;
    }
    out_handle.flush()?;

    Ok(())
}

fn record_vbq_header(paths: &[String]) -> Result<VBinseqHeader> {
    if paths.is_empty() {
        bail!("No input files.");
    }
    let reader = vbq::MmapReader::new(&paths[0])?;
    let header = reader.header();
    Ok(header)
}

fn run_vbq(args: CatCommand) -> Result<()> {
    let out_handle = args.output.as_writer()?;
    let header = record_vbq_header(&args.input.input)?;
    let processor = VBinseqProcessor::new(header, args.output.policy.into(), out_handle)?;
    for path in args.input.input {
        let reader = vbq::MmapReader::new(&path)?;
        reader.process_parallel(processor.clone(), args.output.threads())?;
    }
    processor.finish()?;
    Ok(())
}

pub fn run(args: CatCommand) -> Result<()> {
    if is_all_bq(&args.input.input)? {
        run_bq(args)
    } else {
        run_vbq(args)
    }
}
