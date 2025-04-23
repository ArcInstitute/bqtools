use std::io::Write;

mod decode_binseq;
mod utils;

use anyhow::{bail, Result};
use binseq::prelude::*;
use decode_binseq::Decoder;
pub use utils::{write_record_pair, SplitWriter};

use crate::cli::{DecodeCommand, Mate, OutputFile};

/// Convenience type wrapper
pub type Writer = Box<dyn Write + Send>;

pub fn build_writer(args: &OutputFile, paired: bool) -> Result<SplitWriter> {
    let format = args.format()?;

    // Split writer
    if args.prefix.is_some() {
        if !paired {
            bail!("Cannot split file into two. No extended sequence channel");
        }
        if args.mate == Mate::Both {
            let (r1, r2) = args.as_paired_writer(format)?;
            let split = SplitWriter::new_split(r1, r2);
            Ok(split)
        } else {
            // Interleaved writer
            let writer = args.as_writer()?;
            let split = SplitWriter::new_interleaved(writer);
            Ok(split)
        }
    } else {
        match args.mate {
            Mate::One | Mate::Two => {
                eprintln!("Warning: Ignoring mate as single channel in file");
            }
            Mate::Both => {}
        }
        // Interleaved writer
        let writer = args.as_writer()?;
        let split = SplitWriter::new_interleaved(writer);
        Ok(split)
    }
}

pub fn run(args: &DecodeCommand) -> Result<()> {
    let reader = BinseqReader::new(args.input.path())?;
    let writer = build_writer(&args.output, reader.is_paired())?;
    let format = args.output.format()?;
    let mate = if reader.is_paired() {
        Some(args.output.mate())
    } else {
        None
    };
    let proc = Decoder::new(writer, format, mate);
    reader.process_parallel(proc.clone(), args.output.threads())?;
    let num_records = proc.num_records();
    eprintln!("Processed {num_records} records...");
    Ok(())
}
