use std::{fs::File, io::Write};

use anyhow::{bail, Result};
use binseq::{bq, cbq, vbq, BinseqReader, BinseqWriterBuilder, ParallelReader};
use log::{error, trace, warn};
use memmap2::MmapOptions;

use crate::{
    cli::{BinseqMode, CatCommand},
    commands::encode::processor::Encoder,
};

fn strip_header(path: &str) -> Result<bq::FileHeader> {
    let reader = bq::MmapReader::new(path)?;
    Ok(reader.header())
}

fn recover_header(paths: &[String]) -> Result<bq::FileHeader> {
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

fn determine_mode(paths: &[String]) -> Result<BinseqMode> {
    let mut mode = None;
    for path in paths {
        let reader = BinseqReader::new(path)?;
        if mode.is_none() {
            mode = match reader {
                BinseqReader::Bq(_) => Some(BinseqMode::Bq),
                BinseqReader::Vbq(_) => Some(BinseqMode::Vbq),
                BinseqReader::Cbq(_) => Some(BinseqMode::Cbq),
            };
            trace!("Initializing Mode {:?} for path: {}", mode.unwrap(), path);
        } else {
            match (mode, reader) {
                (Some(BinseqMode::Bq), BinseqReader::Bq(_)) => (),
                (Some(BinseqMode::Vbq), BinseqReader::Vbq(_)) => (),
                (Some(BinseqMode::Cbq), BinseqReader::Cbq(_)) => (),
                _ => bail!(
                    "Inconsistent modes found, expecting the same BINSEQ mode for all input files."
                ),
            }
            trace!("Mode {:?} for path: {}", mode.unwrap(), path);
        }
    }
    mode.ok_or_else(|| anyhow::anyhow!("No input files."))
}

fn run_bq(args: CatCommand) -> Result<()> {
    let header = recover_header(&args.input.input)?;
    let mut out_handle = args.output.as_writer()?;

    header.write_bytes(&mut out_handle)?;
    for path in args.input.input {
        let file = File::open(path)?;
        let mmap = unsafe { MmapOptions::new().map(&file)? };
        out_handle.write_all(&mmap[bq::SIZE_HEADER..])?;
    }
    out_handle.flush()?;

    Ok(())
}

fn record_vbq_header(paths: &[String]) -> Result<vbq::FileHeader> {
    if paths.is_empty() {
        bail!("No input files.");
    }
    let reader = vbq::MmapReader::new(&paths[0])?;
    let header = reader.header();
    for path in &paths[1..] {
        let reader = vbq::MmapReader::new(path)?;
        if reader.header() != header {
            error!("Inconsistent header found for path: {}", path);
            warn!("Note: The first VBQ used in `cat` will be considered as the reference header. All subsequent VBQs must have the same header.");
            bail!("Inconsistent header found for path: {}", path);
        }
    }
    Ok(header)
}

fn record_cbq_header(paths: &[String]) -> Result<cbq::FileHeader> {
    if paths.is_empty() {
        bail!("No paths provided");
    }
    let reader = cbq::MmapReader::new(&paths[0])?;
    let header = reader.header();
    for path in &paths[1..] {
        let reader = cbq::MmapReader::new(path)?;
        if reader.header() != header {
            error!("Inconsistent header found for path: {}", path);
            warn!("Note: The first CBQ used in `cat` will be considered as the reference header. All subsequent CBQs must have the same header.");
            bail!("Inconsistent header found for path: {}", path);
        }
    }
    Ok(header)
}

fn run_cat(args: CatCommand, mode: BinseqMode) -> Result<()> {
    // initialize output handle
    let ohandle = args.output.as_writer()?;

    // initialize writer
    let writer = if matches!(mode, BinseqMode::Vbq) {
        let header = record_vbq_header(&args.input.input)?;
        BinseqWriterBuilder::from_vbq_header(header).build(ohandle)
    } else {
        let header = record_cbq_header(&args.input.input)?;
        BinseqWriterBuilder::from_cbq_header(header).build(ohandle)
    }?;

    // Concatenate
    let mut processor = Encoder::new(writer)?;
    for path in args.input.input {
        let reader = BinseqReader::new(&path)?;
        reader.process_parallel(processor.clone(), args.output.threads())?;
    }
    processor.finish()?;
    Ok(())
}

pub fn run(args: CatCommand) -> Result<()> {
    match determine_mode(&args.input.input)? {
        BinseqMode::Bq => run_bq(args),
        BinseqMode::Vbq => run_cat(args, BinseqMode::Vbq),
        BinseqMode::Cbq => run_cat(args, BinseqMode::Cbq),
    }
}
