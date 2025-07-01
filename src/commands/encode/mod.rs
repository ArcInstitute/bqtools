use anyhow::Result;
use binseq::{bq::BinseqHeader, vbq::VBinseqHeader, Policy};
use paraseq::{
    fastx::{Format, Reader},
    prelude::*,
};

use crate::{
    cli::{BinseqMode, EncodeCommand, TruncateConfig},
    commands::utils::match_output,
};

mod processor;
mod utils;

use processor::{BinseqProcessor, VBinseqProcessor};
use utils::{get_interleaved_sequence_len, get_sequence_len};

fn encode_single(
    in_path: Option<&str>,
    out_path: Option<&str>,
    mode: BinseqMode,
    num_threads: usize,
    compress: bool,
    quality: bool,
    block_size: usize,
    batch_size: Option<usize>,
    policy: Policy,
    truncate: Option<TruncateConfig>,
) -> Result<()> {
    // build reader
    let mut reader = if let Some(size) = batch_size {
        Reader::from_optional_path_with_batch_size(in_path, size)
    } else {
        Reader::from_optional_path(in_path)
    }?;

    // build writer
    let out_handle = match_output(out_path)?;

    let (num_records, num_skipped) = if mode == BinseqMode::Binseq {
        // Determine the sequence length
        let slen = get_sequence_len(&mut reader, truncate, true)?;

        let header = BinseqHeader::new(slen);
        let processor = BinseqProcessor::new(header, policy.into(), truncate, out_handle)?;

        // Process the records in parallel
        reader.process_parallel(processor.clone(), num_threads)?;

        // Update the number of records
        let num_records = processor.get_global_record_count();
        let num_skipped = processor.get_global_skipped_count();

        (num_records, num_skipped)
    } else {
        let quality = match reader.format() {
            Format::Fastq => quality,
            Format::Fasta => false, // never record fasta quality
        };
        let header = VBinseqHeader::with_capacity(block_size as u64, quality, compress, false);
        let processor = VBinseqProcessor::new(header, policy.into(), out_handle)?;

        // Process the records in parallel
        reader.process_parallel(processor.clone(), num_threads)?;
        processor.finish()?;

        // Update the number of records
        let num_records = processor.get_global_record_count();
        let num_skipped = processor.get_global_skipped_count();

        (num_records, num_skipped)
    };

    // print the summary
    eprintln!("{num_records} records written");
    if num_skipped > 0 {
        eprintln!("{num_skipped} records skipped (invalid nucleotides)");
    }

    Ok(())
}

fn encode_interleaved(
    in_path: Option<&str>,
    out_path: Option<&str>,
    mode: BinseqMode,
    num_threads: usize,
    compress: bool,
    quality: bool,
    block_size: usize,
    batch_size: Option<usize>,
    policy: Policy,
    truncate: Option<TruncateConfig>,
) -> Result<()> {
    let mut reader = if let Some(size) = batch_size {
        Reader::from_optional_path_with_batch_size(in_path, size)
    } else {
        Reader::from_optional_path(in_path)
    }?;

    // Prepare the processor
    let out_handle = match_output(out_path)?;

    let (num_records, num_skipped) = if mode == BinseqMode::Binseq {
        // Determine the sequence length
        let (slen, xlen) = get_interleaved_sequence_len(&mut reader, truncate)?;

        let header = BinseqHeader::new_extended(slen, xlen);
        let processor = BinseqProcessor::new(header, policy.into(), truncate, out_handle)?;

        // Process the records in parallel
        reader.process_parallel_interleaved(processor.clone(), num_threads)?;

        // Update the number of records
        let num_records = processor.get_global_record_count();
        let num_skipped = processor.get_global_skipped_count();

        (num_records, num_skipped)
    } else {
        let quality = match reader.format() {
            Format::Fastq => quality,
            Format::Fasta => false, // never record quality for fasta
        };
        let header = VBinseqHeader::with_capacity(block_size as u64, quality, compress, true);
        let processor = VBinseqProcessor::new(header, policy.into(), out_handle)?;

        // Process the records in parallel
        reader.process_parallel_interleaved(processor.clone(), num_threads)?;
        processor.finish()?;

        // Update the number of records
        let num_records = processor.get_global_record_count();
        let num_skipped = processor.get_global_skipped_count();

        (num_records, num_skipped)
    };

    // print the summary
    eprintln!("{num_records} records written");
    if num_skipped > 0 {
        eprintln!("{num_skipped} records skipped (invalid nucleotides)");
    }

    Ok(())
}

fn encode_paired(
    in_path1: &str,
    in_path2: &str,
    out_path: Option<&str>,
    mode: BinseqMode,
    num_threads: usize,
    compress: bool,
    quality: bool,
    block_size: usize,
    batch_size: Option<usize>,
    policy: Policy,
    truncate: Option<TruncateConfig>,
) -> Result<()> {
    let (mut reader_r1, mut reader_r2) = if let Some(size) = batch_size {
        (
            Reader::from_path_with_batch_size(in_path1, size)?,
            Reader::from_path_with_batch_size(in_path2, size)?,
        )
    } else {
        (Reader::from_path(in_path1)?, Reader::from_path(in_path2)?)
    };

    // Prepare the output handle
    let out_handle = match_output(out_path)?;

    let (num_records, num_skipped) = match mode {
        BinseqMode::Binseq => {
            // Determine the sequence length
            let slen = get_sequence_len(&mut reader_r1, truncate, true)?;
            let xlen = get_sequence_len(&mut reader_r2, truncate, false)?;

            // Prepare the output HEADER
            let header = BinseqHeader::new_extended(slen, xlen);
            let processor = BinseqProcessor::new(header, policy.into(), truncate, out_handle)?;

            // Process the records in parallel
            reader_r1.process_parallel_paired(reader_r2, processor.clone(), num_threads)?;

            // Update the number of records
            let num_records = processor.get_global_record_count();
            let num_skipped = processor.get_global_skipped_count();

            (num_records, num_skipped)
        }
        BinseqMode::VBinseq => {
            let quality = match reader_r1.format() {
                Format::Fastq => quality,
                Format::Fasta => false, // never record quality for fasta
            };
            let header = VBinseqHeader::with_capacity(block_size as u64, quality, compress, true);
            let processor = VBinseqProcessor::new(header, policy.into(), out_handle)?;

            // Process the records in parallel
            reader_r1.process_parallel_paired(reader_r2, processor.clone(), num_threads)?;
            processor.finish()?;

            // Update the number of records
            let num_records = processor.get_global_record_count();
            let num_skipped = processor.get_global_skipped_count();

            (num_records, num_skipped)
        }
    };

    // print the summary
    eprintln!("{num_records} record pairs written");
    if num_skipped > 0 {
        eprintln!("{num_skipped} record pairs skipped (invalid nucleotides)");
    }

    Ok(())
}

pub fn run(args: &EncodeCommand) -> Result<()> {
    if args.input.paired() {
        let (in_path1, in_path2) = args.input.paired_paths()?;
        encode_paired(
            in_path1,
            in_path2,
            args.output.borrowed_path(),
            args.output.mode()?,
            args.output.threads(),
            args.output.compress(),
            args.output.quality(),
            args.output.block_size,
            args.input.batch_size,
            args.output.policy.into(),
            args.output.truncate_config(),
        )?;
    } else if args.input.interleaved {
        encode_interleaved(
            args.input.single_path()?,
            args.output.borrowed_path(),
            args.output.mode()?,
            args.output.threads(),
            args.output.compress(),
            args.output.quality(),
            args.output.block_size,
            args.input.batch_size,
            args.output.policy.into(),
            args.output.truncate_config(),
        )?;
    } else {
        encode_single(
            args.input.single_path()?,
            args.output.borrowed_path(),
            args.output.mode()?,
            args.output.threads(),
            args.output.compress(),
            args.output.quality(),
            args.output.block_size,
            args.input.batch_size,
            args.output.policy.into(),
            args.output.truncate_config(),
        )?;
    }

    if args.output.index
        && args.output.mode()? == BinseqMode::VBinseq
        && args.output.output.is_some()
    {
        crate::commands::index::index_path(args.output.borrowed_path().unwrap(), true)?;
    }

    Ok(())
}
