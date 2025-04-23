use std::io::Read;

use anyhow::Result;
use binseq::{bq::BinseqHeader, vbq::VBinseqHeader};
use paraseq::{
    fastq::Reader,
    parallel::{InterleavedParallelReader, PairedParallelReader, ParallelReader},
};

use super::{get_sequence_len_fastq, BinseqProcessor, VBinseqProcessor};
use crate::{
    cli::{BinseqMode, PolicyWrapper},
    commands::{encode::utils::get_interleaved_sequence_len_fastq, utils::match_output},
};

#[allow(clippy::too_many_arguments)]
pub fn encode_single_fastq_parallel(
    in_handle: Box<dyn Read + Send>,
    out_path: Option<&str>,
    num_threads: usize,
    policy: PolicyWrapper,
    mode: BinseqMode,
    compress: bool,
    quality: bool,
    block_size: usize,
) -> Result<()> {
    // Open the input FASTQ file
    let mut reader = Reader::new(in_handle);

    // Prepare the processor
    let out_handle = match_output(out_path)?;

    let (num_records, num_skipped) = if mode == BinseqMode::Binseq {
        // Determine the sequence length
        let slen = get_sequence_len_fastq(&mut reader)?;

        let header = BinseqHeader::new(slen);
        let processor = BinseqProcessor::new(header, policy.into(), out_handle)?;

        // Process the records in parallel
        reader.process_parallel(processor.clone(), num_threads)?;

        // Update the number of records
        let num_records = processor.get_global_record_count();
        let num_skipped = processor.get_global_skipped_count();

        (num_records, num_skipped)
    } else {
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

#[allow(clippy::too_many_arguments)]
pub fn encode_interleaved_fastq_parallel(
    in_handle: Box<dyn Read + Send>,
    out_path: Option<&str>,
    num_threads: usize,
    policy: PolicyWrapper,
    mode: BinseqMode,
    compress: bool,
    quality: bool,
    block_size: usize,
) -> Result<()> {
    // Open the input FASTQ file
    let mut reader = Reader::new(in_handle);

    // Prepare the processor
    let out_handle = match_output(out_path)?;

    let (num_records, num_skipped) = if mode == BinseqMode::Binseq {
        // Determine the sequence length
        let (slen, xlen) = get_interleaved_sequence_len_fastq(&mut reader)?;

        let header = BinseqHeader::new_extended(slen, xlen);
        let processor = BinseqProcessor::new(header, policy.into(), out_handle)?;

        // Process the records in parallel
        reader.process_parallel_interleaved(processor.clone(), num_threads)?;

        // Update the number of records
        let num_records = processor.get_global_record_count();
        let num_skipped = processor.get_global_skipped_count();

        (num_records, num_skipped)
    } else {
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

#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_arguments)]
pub fn encode_paired_fastq_parallel(
    r1_handle: Box<dyn Read + Send>,
    r2_handle: Box<dyn Read + Send>,
    out_path: Option<&str>,
    num_threads: usize,
    policy: PolicyWrapper,
    mode: BinseqMode,
    compress: bool,
    quality: bool,
    block_size: usize,
) -> Result<()> {
    // Open the input FASTQ files
    let mut reader_r1 = Reader::new(r1_handle);
    let mut reader_r2 = Reader::new(r2_handle);

    // Prepare the output handle
    let out_handle = match_output(out_path)?;

    let (num_records, num_skipped) = match mode {
        BinseqMode::Binseq => {
            // Determine the sequence length
            let slen = get_sequence_len_fastq(&mut reader_r1)?;
            let xlen = get_sequence_len_fastq(&mut reader_r2)?;

            // Prepare the output HEADER
            let header = BinseqHeader::new_extended(slen, xlen);
            let processor = BinseqProcessor::new(header, policy.into(), out_handle)?;

            // Process the records in parallel
            reader_r1.process_parallel_paired(reader_r2, processor.clone(), num_threads)?;

            // Update the number of records
            let num_records = processor.get_global_record_count();
            let num_skipped = processor.get_global_skipped_count();

            (num_records, num_skipped)
        }
        BinseqMode::VBinseq => {
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
