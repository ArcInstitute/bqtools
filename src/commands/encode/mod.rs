mod encode_macros;
mod parallel;

use parallel::Processor;

use anyhow::{bail, Result};
use std::io::Read;

use crate::{
    cli::{EncodeCommand, FileFormat},
    encode_paired_fastx, encode_single_fastx,
};

fn encode_single(args: EncodeCommand) -> Result<()> {
    // Open the IO handles
    let in_handle = args.input.as_reader()?;
    let out_handle = args.output.as_writer()?;

    // Compression passthrough on input
    let (in_handle, _comp) = niffler::get_reader(in_handle)?;

    match args.input.format()? {
        FileFormat::Fastq => {
            encode_single_fastx!(seq_io::fastq::Reader<Box<dyn Read>>, in_handle, out_handle)
        }
        FileFormat::Fasta => {
            encode_single_fastx!(seq_io::fasta::Reader<Box<dyn Read>>, in_handle, out_handle)
        }
    }
}

fn encode_paired(args: EncodeCommand) -> Result<()> {
    // Open the IO handles
    let (r1_handle, r2_handle) = args.input.as_reader_pair()?;

    // Compression passthrough on input
    let (r1_handle, _comp) = niffler::send::get_reader(r1_handle)?;
    let (r2_handle, _comp) = niffler::send::get_reader(r2_handle)?;

    match args.input.format()? {
        FileFormat::Fastq => {
            if args.output.threads() > 1 {
                encode_paired_fastx!(
                    seq_io::fastq::Reader<Box<dyn Read + Send>>,
                    r1_handle,
                    r2_handle,
                    args.output.as_writer()?,
                    args.output.owned_path(),
                    args.output.threads()
                )
            } else {
                encode_paired_fastx!(
                    seq_io::fastq::Reader<Box<dyn Read>>,
                    r1_handle,
                    r2_handle,
                    args.output.as_writer()?
                )
            }
        }
        FileFormat::Fasta => {
            if args.output.threads() > 1 {
                encode_paired_fastx!(
                    seq_io::fasta::Reader<Box<dyn Read + Send>>,
                    r1_handle,
                    r2_handle,
                    args.output.as_writer()?,
                    args.output.owned_path(),
                    args.output.threads()
                )
            } else {
                encode_paired_fastx!(
                    seq_io::fasta::Reader<Box<dyn Read>>,
                    r1_handle,
                    r2_handle,
                    args.output.as_writer()?
                )
            }
        }
    }
}

pub fn run(args: EncodeCommand) -> Result<()> {
    if args.input.paired() {
        encode_paired(args)
    } else {
        encode_single(args)
    }
}
