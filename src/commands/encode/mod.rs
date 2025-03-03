use anyhow::Result;

use crate::cli::{EncodeCommand, FileFormat};

mod fasta;
mod fastq;
mod processor;

use fasta::{encode_paired_fasta_parallel, encode_single_fasta_parallel};
use fastq::{encode_paired_fastq_parallel, encode_single_fastq_parallel};
use processor::Processor;

fn encode_single(args: EncodeCommand) -> Result<()> {
    // Open the IO handles
    let in_handle = args.input.as_reader()?;

    // Compression passthrough on input
    let (in_handle, _comp) = niffler::send::get_reader(in_handle)?;

    match args.input.format()? {
        FileFormat::Fastq => encode_single_fastq_parallel(
            in_handle,
            args.output.owned_path(),
            args.output.threads(),
            args.output.policy(),
        ),
        FileFormat::Fasta => encode_single_fasta_parallel(
            in_handle,
            args.output.owned_path(),
            args.output.threads(),
            args.output.policy(),
        ),
        _ => {
            unimplemented!("Tsv import is not implemented for encoding");
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
        FileFormat::Fastq => encode_paired_fastq_parallel(
            r1_handle,
            r2_handle,
            args.output.owned_path(),
            args.output.threads(),
            args.output.policy(),
        ),
        FileFormat::Fasta => encode_paired_fasta_parallel(
            r1_handle,
            r2_handle,
            args.output.owned_path(),
            args.output.threads(),
            args.output.policy(),
        ),
        _ => {
            unimplemented!("Tsv import is not implemented for encoding")
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
