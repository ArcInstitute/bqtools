mod process;

use anyhow::Result;
use std::io::Read;

use crate::{
    cli::{EncodeCommand, FileFormat},
    process_paired_fastx, process_single_fastx,
};

fn encode_single(args: EncodeCommand) -> Result<()> {
    // Open the IO handles
    let in_handle = args.input.as_reader()?;
    let out_handle = args.output.as_writer()?;

    // Compression passthrough on input
    let (in_handle, _comp) = niffler::get_reader(in_handle)?;

    match args.input.format()? {
        FileFormat::Fastq => {
            process_single_fastx!(seq_io::fastq::Reader<Box<dyn Read>>, in_handle, out_handle)
        }
        FileFormat::Fasta => {
            process_single_fastx!(seq_io::fasta::Reader<Box<dyn Read>>, in_handle, out_handle)
        }
    }
}

fn encode_paired(args: EncodeCommand) -> Result<()> {
    // Open the IO handles
    let (r1_handle, r2_handle) = args.input.as_reader_pair()?;
    let out_handle = args.output.as_writer()?;

    // Compression passthrough on input
    let (r1_handle, _comp) = niffler::get_reader(r1_handle)?;
    let (r2_handle, _comp) = niffler::get_reader(r2_handle)?;

    match args.input.format()? {
        FileFormat::Fastq => {
            process_paired_fastx!(
                seq_io::fastq::Reader<Box<dyn Read>>,
                r1_handle,
                r2_handle,
                out_handle
            )
        }
        FileFormat::Fasta => {
            process_paired_fastx!(
                seq_io::fasta::Reader<Box<dyn Read>>,
                r1_handle,
                r2_handle,
                out_handle
            )
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
