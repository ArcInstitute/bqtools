use anyhow::Result;

use crate::cli::{BinseqMode, EncodeCommand, FileFormat};

mod fasta;
mod fastq;
mod processor;
mod utils;

use fasta::{
    encode_interleaved_fasta_parallel, encode_paired_fasta_parallel, encode_single_fasta_parallel,
};
use fastq::{
    encode_interleaved_fastq_parallel, encode_paired_fastq_parallel, encode_single_fastq_parallel,
};
use processor::{BinseqProcessor, VBinseqProcessor};
use utils::{get_sequence_len_fasta, get_sequence_len_fastq};

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
            args.output.policy,
            args.output.mode()?,
            args.output.compress(),
            args.output.quality(),
            args.output.block_size,
        ),
        FileFormat::Fasta => encode_single_fasta_parallel(
            in_handle,
            args.output.owned_path(),
            args.output.threads(),
            args.output.policy,
            args.output.mode()?,
            args.output.compress(),
            args.output.block_size,
        ),
        _ => {
            unimplemented!("Tsv import is not implemented for encoding");
        }
    }
}

fn encode_interleaved(args: EncodeCommand) -> Result<()> {
    // Open the IO handles
    let in_handle = args.input.as_reader()?;

    // Compression passthrough on input
    let (in_handle, _comp) = niffler::send::get_reader(in_handle)?;

    match args.input.format()? {
        FileFormat::Fastq => encode_interleaved_fastq_parallel(
            in_handle,
            args.output.owned_path(),
            args.output.threads(),
            args.output.policy,
            args.output.mode()?,
            args.output.compress(),
            args.output.quality(),
            args.output.block_size,
        ),
        FileFormat::Fasta => encode_interleaved_fasta_parallel(
            in_handle,
            args.output.owned_path(),
            args.output.threads(),
            args.output.policy,
            args.output.mode()?,
            args.output.compress(),
            args.output.block_size,
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
            args.output.policy,
            args.output.mode()?,
            args.output.compress(),
            args.output.quality(),
            args.output.block_size,
        ),
        FileFormat::Fasta => encode_paired_fasta_parallel(
            r1_handle,
            r2_handle,
            args.output.owned_path(),
            args.output.threads(),
            args.output.policy,
            args.output.mode()?,
            args.output.compress(),
            args.output.block_size,
        ),
        _ => {
            unimplemented!("Tsv import is not implemented for encoding")
        }
    }
}

pub fn run(args: EncodeCommand) -> Result<()> {
    if args.input.paired() {
        encode_paired(args.clone())?;
    } else if args.input.interleaved {
        encode_interleaved(args.clone())?;
    } else {
        encode_single(args.clone())?;
    }

    if args.output.index
        && args.output.mode()? == BinseqMode::VBinseq
        && args.output.output.is_some()
    {
        crate::commands::index::index_path(&args.output.output.unwrap(), true)?;
    }

    Ok(())
}
