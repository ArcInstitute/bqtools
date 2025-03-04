use std::io::Read;

use anyhow::Result;
use binseq::{BinseqHeader, Policy};
use paraseq::{
    fasta::Reader,
    parallel::{PairedParallelReader, ParallelReader},
};

use super::{get_sequence_len_fasta, Processor};
use crate::commands::utils::match_output;

pub fn encode_single_fasta_parallel(
    in_handle: Box<dyn Read + Send>,
    out_path: Option<String>,
    num_threads: usize,
    policy: Policy,
) -> Result<()> {
    // Open the input fasta file
    let mut reader = Reader::new(in_handle);

    // Determine the sequence length
    let slen = get_sequence_len_fasta(&mut reader)?;

    // Prepare the processor
    let out_handle = match_output(out_path.as_ref())?;
    let header = BinseqHeader::new(slen);
    let processor = Processor::new(header, policy, out_handle)?;

    // Process the records in parallel
    reader.process_parallel(processor.clone(), num_threads)?;

    // Recover the number of records
    let num_records = processor.get_global_record_count();
    let num_skipped = processor.get_global_skipped_count();

    // print the summary
    eprintln!("{} records written", num_records);
    if num_skipped > 0 {
        eprintln!("{} records skipped (invalid nucleotides)", num_skipped);
    }

    Ok(())
}

pub fn encode_paired_fasta_parallel(
    r1_handle: Box<dyn Read + Send>,
    r2_handle: Box<dyn Read + Send>,
    out_path: Option<String>,
    num_threads: usize,
    policy: Policy,
) -> Result<()> {
    // Open the input fasta file
    let mut reader_r1 = Reader::new(r1_handle);
    let mut reader_r2 = Reader::new(r2_handle);

    // Determine the length of the sequences
    let slen = get_sequence_len_fasta(&mut reader_r1)?;
    let xlen = get_sequence_len_fasta(&mut reader_r2)?;

    // Prepare the processor
    let out_handle = match_output(out_path.as_ref())?;
    let header = BinseqHeader::new_extended(slen, xlen);
    let processor = Processor::new(header, policy, out_handle)?;

    // Process the records in parallel
    reader_r1.process_parallel_paired(reader_r2, processor.clone(), num_threads)?;

    // Recover the number of records
    let num_records = processor.get_global_record_count();
    let num_skipped = processor.get_global_skipped_count();

    // print the summary
    eprintln!("{} record pairs written", num_records);
    if num_skipped > 0 {
        eprintln!("{} record pairs skipped (invalid nucleotides)", num_skipped);
    }

    Ok(())
}
