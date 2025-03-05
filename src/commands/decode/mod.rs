use std::io::Write;

mod decode_binseq;
mod utils;

use anyhow::{bail, Result};
use binseq::MmapReader;
use decode_binseq::Decoder;
use utils::{write_record_pair, SplitWriter};

use crate::cli::{BinseqMode, DecodeCommand, Mate};

/// Convenience type wrapper
pub type Writer = Box<dyn Write + Send>;

fn build_writer(args: &DecodeCommand, paired: bool) -> Result<SplitWriter> {
    let format = args.output.format()?;

    // Split writer
    if args.output.prefix.is_some() {
        if !paired {
            bail!("Cannot split file into two. No extended sequence channel");
        }
        match args.output.mate {
            Mate::Both => {
                let (r1, r2) = args.output.as_paired_writer(format)?;
                let split = SplitWriter::new_split(r1, r2);
                Ok(split)
            }
            _ => {
                eprintln!("Warning: Ignoring prefix as mate was provided");
                // Interleaved writer
                let writer = args.output.as_writer()?;
                let split = SplitWriter::new_interleaved(writer);
                Ok(split)
            }
        }
    } else {
        match args.output.mate {
            Mate::One | Mate::Two => {
                eprintln!("Warning: Ignoring mate as single channel in file");
            }
            _ => {}
        }
        // Interleaved writer
        let writer = args.output.as_writer()?;
        let split = SplitWriter::new_interleaved(writer);
        Ok(split)
    }
}

pub fn run(args: DecodeCommand) -> Result<()> {
    let num_records = match args.input.mode()? {
        BinseqMode::Binseq => {
            let reader = MmapReader::new(args.input.path())?;
            let writer = build_writer(&args, reader.header().xlen > 0)?;
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
            let reader = vbinseq::MmapReader::new(args.input.path())?;
            let writer = build_writer(&args, reader.header().paired)?;
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
