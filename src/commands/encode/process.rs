#[macro_export]
macro_rules! process_single_fastx {
    ($reader:ty, $in_handle:expr, $writer:expr) => {{
        use anyhow::bail;
        use binseq::{BinseqHeader, BinseqWriter};

        #[allow(unused_imports)]
        use seq_io::{fasta::Record as FastaRecord, fastq::Record as FastqRecord};

        // Open the input FASTX file
        let mut reader = <$reader>::new($in_handle);
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
        let mut writer = BinseqWriter::new($writer, header, false)?;

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
    }};
}
