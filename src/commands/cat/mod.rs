use std::{fs::File, io::Write};

use anyhow::{bail, Result};
use binseq::{BinseqHeader, BinseqRead, MmapReader, SIZE_HEADER};
use memmap2::MmapOptions;

use crate::cli::CatCommand;

fn recover_header(paths: &[String]) -> Result<BinseqHeader> {
    let mut header = None;
    for path in paths {
        let reader = MmapReader::new(path)?;
        if let Some(h) = header {
            if h != reader.header() {
                bail!("Inconsistent headers.");
            }
        } else {
            header = Some(reader.header());
        }
    }
    header.ok_or_else(|| anyhow::anyhow!("No input files."))
}

pub fn run(args: CatCommand) -> Result<()> {
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
