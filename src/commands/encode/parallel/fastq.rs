use std::io::Read;

use anyhow::{bail, Result};
use binseq::{writer::Policy, BinseqHeader, BinseqWriter};
use seq_io_parallel::{
    fastq::{Reader, Record},
    PairedParallelReader, ParallelReader,
};

use super::Processor;
use crate::commands::utils::match_output;

pub fn encode_single_fastq_parallel(
    in_handle: Box<dyn Read + Send>,
    out_path: Option<String>,
    num_threads: usize,
    policy: Policy,
) -> Result<()> {
    // Open the input FASTQ file
    let mut reader = Reader::new(in_handle);
    let mut num_records = 0;
    let mut num_skipped = 0;

    // Get the first record for the expected size of the sequences
    let seq = if let Some(record) = reader.next() {
        let record = record?;
        let seq = record.seq();
        seq.to_vec()
    } else {
        bail!("Input file is empty - cannot convert");
    };

    // Prepare the output HEADER
    let header = BinseqHeader::new(seq.len() as u32);

    // Write the header and the first sequence
    {
        // Prepare the output handle
        let out_handle = match_output(out_path.as_ref())?;

        // Prepare the output WRITER and write the header
        let mut writer = BinseqWriter::new_with_policy(out_handle, header, policy)?;

        // Write the first sequence
        if writer.write_nucleotides(0, &seq)? {
            num_records += 1;
        } else {
            num_skipped += 1;
        }

        // Flush the writer
        writer.flush()?;
    }

    // Process the remaining records with parallel processing
    let processor = Processor::new(header, out_path, policy);
    reader.process_parallel(processor.clone(), num_threads)?;

    // Update the number of records
    num_records += processor.get_global_num_records();
    num_skipped += processor.get_global_num_skipped();

    // print the summary
    eprintln!("{} records written", num_records);
    if num_skipped > 0 {
        eprintln!("{} records skipped (invalid nucleotides)", num_skipped);
    }

    Ok(())
}

pub fn encode_paired_fastq_parallel(
    r1_handle: Box<dyn Read + Send>,
    r2_handle: Box<dyn Read + Send>,
    out_path: Option<String>,
    num_threads: usize,
    policy: Policy,
) -> Result<()> {
    // Open the input FASTQ file
    let mut reader_r1 = Reader::new(r1_handle);
    let mut reader_r2 = Reader::new(r2_handle);
    let mut num_records = 0;
    let mut num_skipped = 0;

    // Get the first record for the expected size of the sequences
    let seq1 = if let Some(record) = reader_r1.next() {
        let record = record?;
        let seq = record.seq();
        seq.to_vec()
    } else {
        bail!("Input file is empty - cannot convert");
    };
    let seq2 = if let Some(record) = reader_r2.next() {
        let record = record?;
        let seq = record.seq();
        seq.to_vec()
    } else {
        bail!("Input file is empty - cannot convert");
    };

    // Prepare the output HEADER
    let header = BinseqHeader::new_extended(seq1.len() as u32, seq2.len() as u32);

    // Write the header and the first sequence pair
    {
        // Prepare the output handle
        let out_handle = match_output(out_path.as_ref())?;

        // Prepare the output WRITER and write the header
        let mut writer = BinseqWriter::new_with_policy(out_handle, header, policy)?;

        // Write the first sequence pair
        if writer.write_paired(0, &seq1, &seq2)? {
            num_records += 1;
        } else {
            num_skipped += 1;
        }

        // Flush the writer
        writer.flush()?;
    }

    // Process the remaining records with parallel processing
    let processor = Processor::new(header, out_path, policy);
    reader_r1.process_parallel_paired(reader_r2, processor.clone(), num_threads)?;

    // Update the number of records
    num_records += processor.get_global_num_records();
    num_skipped += processor.get_global_num_skipped();

    // print the summary
    eprintln!("{} record pairs written", num_records);
    if num_skipped > 0 {
        eprintln!("{} record pairs skipped (invalid nucleotides)", num_skipped);
    }

    Ok(())
}
