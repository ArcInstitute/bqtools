mod encodings;
mod parallel;

use anyhow::Result;

use crate::cli::{EncodeCommand, FileFormat};
use encodings::{
    encode_paired_fasta, encode_paired_fastq, encode_single_fasta, encode_single_fastq,
};
use parallel::{
    encode_paired_fasta_parallel, encode_paired_fastq_parallel, encode_single_fasta_parallel,
    encode_single_fastq_parallel,
};

fn encode_single(args: EncodeCommand) -> Result<()> {
    // Open the IO handles
    let in_handle = args.input.as_reader()?;

    // Compression passthrough on input
    let (in_handle, _comp) = niffler::send::get_reader(in_handle)?;

    match args.input.format()? {
        FileFormat::Fastq => {
            if args.output.threads() > 1 {
                encode_single_fastq_parallel(
                    in_handle,
                    args.output.owned_path(),
                    args.output.threads(),
                    args.output.policy(),
                )
            } else {
                encode_single_fastq(in_handle, args.output.as_writer()?, args.output.policy())
            }
        }
        FileFormat::Fasta => {
            if args.output.threads() > 1 {
                encode_single_fasta_parallel(
                    in_handle,
                    args.output.owned_path(),
                    args.output.threads(),
                    args.output.policy(),
                )
            } else {
                encode_single_fasta(in_handle, args.output.as_writer()?, args.output.policy())
            }
        }
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
        FileFormat::Fastq => {
            if args.output.threads() > 1 {
                encode_paired_fastq_parallel(
                    r1_handle,
                    r2_handle,
                    args.output.owned_path(),
                    args.output.threads(),
                    args.output.policy(),
                )
            } else {
                encode_paired_fastq(
                    r1_handle,
                    r2_handle,
                    args.output.as_writer()?,
                    args.output.policy(),
                )
            }
        }
        FileFormat::Fasta => {
            if args.output.threads() > 1 {
                encode_paired_fasta_parallel(
                    r1_handle,
                    r2_handle,
                    args.output.owned_path(),
                    args.output.threads(),
                    args.output.policy(),
                )
            } else {
                encode_paired_fasta(
                    r1_handle,
                    r2_handle,
                    args.output.as_writer()?,
                    args.output.policy(),
                )
            }
        }
        _ => {
            unimplemented!()
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
