use std::io::Write;

mod decode_binseq;
mod utils;

use anyhow::{bail, Result};
use binseq::{bq, prelude::*, vbq};
use decode_binseq::Decoder;
pub use utils::{write_record_pair, SplitWriter};

use crate::cli::{BinseqMode, DecodeCommand, Mate, OutputFile};

/// Convenience type wrapper
pub type Writer = Box<dyn Write + Send>;

pub fn build_writer(args: &OutputFile, paired: bool) -> Result<SplitWriter> {
    let format = args.format()?;

    // Split writer
    if args.prefix.is_some() {
        if !paired {
            bail!("Cannot split file into two. No extended sequence channel");
        }
        match args.mate {
            Mate::Both => {
                let (r1, r2) = args.as_paired_writer(format)?;
                let split = SplitWriter::new_split(r1, r2);
                Ok(split)
            }
            _ => {
                eprintln!("Warning: Ignoring prefix as mate was provided");
                // Interleaved writer
                let writer = args.as_writer()?;
                let split = SplitWriter::new_interleaved(writer);
                Ok(split)
            }
        }
    } else {
        match args.mate {
            Mate::One | Mate::Two => {
                eprintln!("Warning: Ignoring mate as single channel in file");
            }
            _ => {}
        }
        // Interleaved writer
        let writer = args.as_writer()?;
        let split = SplitWriter::new_interleaved(writer);
        Ok(split)
    }
}

pub fn run(args: DecodeCommand) -> Result<()> {
    let num_records = match args.input.mode()? {
        BinseqMode::Binseq => {
            let reader = bq::MmapReader::new(args.input.path())?;
            let writer = build_writer(&args.output, reader.header().xlen > 0)?;
            let format = args.output.format()?;
            let mate = if reader.header().xlen > 0 {
                Some(args.output.mate())
            } else {
                None
            };
            let proc = Decoder::new(writer, format, mate);
            reader.process_parallel(proc.clone(), args.output.threads())?;
            proc.num_records()
        }
        _ => {
            let reader = vbq::MmapReader::new(args.input.path())?;
            let writer = build_writer(&args.output, reader.header().paired)?;
            let format = args.output.format()?;
            let mate = if reader.header().paired {
                Some(args.output.mate())
            } else {
                None
            };
            let proc = Decoder::new(writer, format, mate);
            reader.process_parallel(proc.clone(), args.output.threads())?;
            proc.num_records()
        }
    };

    eprintln!("Processed {} records...", num_records);
    Ok(())
}
