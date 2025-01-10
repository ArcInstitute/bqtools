#[macro_export]
macro_rules! encode_single_fastx {
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
    ($reader:ty, $in_handle:expr, $writer:expr, $writer_path:expr, $num_threads:expr) => {{
        use anyhow::bail;
        use binseq::{BinseqHeader, BinseqWriter};
        use seq_io_parallel::ParallelReader;

        use $crate::commands::encode::Processor;

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

        // Write the header and first sequence
        {
            let mut writer = BinseqWriter::new($writer, header, false)?;

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
        let processor = Processor::new(header, $writer_path);
        reader.process_parallel(processor.clone(), $num_threads)?;

        // Update the number of records
        num_records += processor.get_global_num_records();
        num_skipped += processor.get_global_num_skipped();

        // print the summary
        eprintln!("{} records written", num_records);
        if num_skipped > 0 {
            eprintln!("{} records skipped (invalid nucleotides)", num_skipped);
        }

        Ok(())
    }};
}

#[macro_export]
macro_rules! encode_paired_fastx {
    ($reader:ty, $r1_handle:expr, $r2_handle:expr, $writer:expr) => {{
        use anyhow::bail;
        use binseq::{BinseqHeader, BinseqWriter};

        #[allow(unused_imports)]
        use seq_io::{fasta::Record as FastaRecord, fastq::Record as FastqRecord};

        // Open the input FASTX file
        let mut reader_r1 = <$reader>::new($r1_handle);
        let mut reader_r2 = <$reader>::new($r2_handle);
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
        let mut writer = BinseqWriter::new($writer, header, false)?;

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
                    if writer.write_paired(0, seq_r1, seq_r2)? {
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
        eprintln!("{} record pairs written", num_records);
        if num_skipped > 0 {
            eprintln!("{} record pairs skipped (invalid nucleotides)", num_skipped);
        }

        Ok(())
    }};
    ($reader:ty, $r1_handle:expr, $r2_handle:expr, $writer:expr, $writer_path:expr, $num_threads:expr) => {{
        use anyhow::bail;
        use binseq::{BinseqHeader, BinseqWriter};
        use seq_io_parallel::PairedParallelReader;

        use $crate::commands::encode::Processor;

        #[allow(unused_imports)]
        use seq_io::{fasta::Record as FastaRecord, fastq::Record as FastqRecord};

        // Open the input FASTX file
        let mut reader_r1 = <$reader>::new($r1_handle);
        let mut reader_r2 = <$reader>::new($r2_handle);
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

        // Write the header and first sequence
        {
            let mut writer = BinseqWriter::new($writer, header, false)?;

            // Write the first sequence
            if writer.write_paired(0, &seq_r1, &seq_r2)? {
                num_records += 1;
            } else {
                num_skipped += 1;
            }

            // Flush the writer
            writer.flush()?;
        }

        // Process the remaining records with parallel processing
        let processor = Processor::new(header, $writer_path);
        reader_r1.process_parallel_paired(reader_r2, processor.clone(), $num_threads)?;

        // Update the number of records
        num_records += processor.get_global_num_records();
        num_skipped += processor.get_global_num_skipped();

        // print the summary
        eprintln!("{} record pairs written", num_records);
        if num_skipped > 0 {
            eprintln!("{} record pairs skipped (invalid nucleotides)", num_skipped);
        }

        Ok(())
    }};
}
