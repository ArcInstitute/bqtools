use anyhow::{bail, Result};
use binseq::{BinseqHeader, BinseqWriter};
use seq_io::fastq::{Reader, Record};

use crate::cli::FastqImport;

pub fn run(args: FastqImport) -> Result<()> {
    let in_handle = args.input.as_reader()?;
    let out_handle = args.output.as_writer()?;

    // Open the input FASTQ
    let (in_handle, _comp) = niffler::get_reader(in_handle)?;

    // Prepare the FASTQ reader
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
    let mut writer = BinseqWriter::new(out_handle, header, false)?;

    // Write the first sequence
    if writer.write_nucleotides(0, &seq)? {
        num_records += 1;
    } else {
        num_skipped += 1;
    };

    // Write the rest of the sequences
    while let Some(record) = reader.next() {
        let record = record?;
        let seq = record.seq();
        if writer.write_nucleotides(num_records + 1, seq)? {
            num_records += 1;
        } else {
            num_skipped += 1;
        }
    }

    // Finalize the writer
    writer.flush()?;

    // Print the summary
    eprintln!("{} records written", num_records);
    if num_skipped > 0 {
        eprintln!("{} records skipped (invalid nucleotides)", num_skipped);
    }

    Ok(())
}
