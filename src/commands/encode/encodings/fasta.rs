use std::io::{Read, Write};

use anyhow::{bail, Result};
use binseq::{BinseqHeader, BinseqWriter, Policy};
use seq_io_parallel::fasta::{Reader, Record};

/// Encode a single FASTA file into a binary format
pub fn encode_single_fasta<W: Write>(
    in_handle: Box<dyn Read>,
    out_handle: W,
    policy: Policy,
) -> Result<()> {
    // Open the input FASTA file
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

    // Prepare the output WRITER
    let mut writer = BinseqWriter::new_with_policy(out_handle, header, policy)?;

    // write the first sequence
    if writer.write_nucleotides(0, &seq)? {
        num_records += 1;
    } else {
        num_skipped += 1;
    };

    // write the remaining sequences
    while let Some(record) = reader.next() {
        let record = record?;
        let seq = record.seq();
        if writer.write_nucleotides(num_records + 1, seq)? {
            num_records += 1;
        } else {
            num_skipped += 1;
        }
    }

    // finalize the writer
    writer.flush()?;

    // print the summary
    eprintln!("{} records written", num_records);
    if num_skipped > 0 {
        eprintln!("{} records skipped (invalid nucleotides)", num_skipped);
    }

    Ok(())
}

/// Encode a pair of FASTA files into a binary format
pub fn encode_paired_fasta<W: Write>(
    r1_handle: Box<dyn Read>,
    r2_handle: Box<dyn Read>,
    out_handle: W,
    policy: Policy,
) -> Result<()> {
    // Open the input FASTA files
    let mut reader_r1 = Reader::new(r1_handle);
    let mut reader_r2 = Reader::new(r2_handle);
    let mut num_records = 0;
    let mut num_skipped = 0;

    // Get the first record for the expected size of the sequences
    let seq_r1 = if let Some(record) = reader_r1.next() {
        let record = record?;
        let seq = record.seq();
        seq.to_vec()
    } else {
        bail!("Input file is empty (R1) - cannot convert");
    };

    let seq_r2 = if let Some(record) = reader_r2.next() {
        let record = record?;
        let seq = record.seq();
        seq.to_vec()
    } else {
        bail!("Input file is empty (R2) - cannot convert");
    };

    // Prepare the output HEADER
    let header = BinseqHeader::new_extended(seq_r1.len() as u32, seq_r2.len() as u32);

    // Prepare the output WRITER
    let mut writer = BinseqWriter::new_with_policy(out_handle, header, policy)?;

    // write the first sequence pair
    if writer.write_paired(0, &seq_r1, &seq_r2)? {
        num_records += 1;
    } else {
        num_skipped += 1;
    };

    // write the remaining sequences
    loop {
        match (reader_r1.next(), reader_r2.next()) {
            (Some(record_r1), Some(record_r2)) => {
                let record_r1 = record_r1?;
                let record_r2 = record_r2?;
                let seq_r1 = record_r1.seq();
                let seq_r2 = record_r2.seq();
                if writer.write_paired(num_records + 1, seq_r1, seq_r2)? {
                    num_records += 1;
                } else {
                    num_skipped += 1;
                }
            }
            (None, None) => break,
            _ => bail!("Input files have different number of records"),
        }
    }

    // finalize the writer
    writer.flush()?;

    // print the summary
    eprintln!("{} records written", num_records);
    if num_skipped > 0 {
        eprintln!("{} records skipped (invalid nucleotides)", num_skipped);
    }

    Ok(())
}
