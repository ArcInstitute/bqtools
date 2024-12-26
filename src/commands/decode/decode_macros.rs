#[macro_export]
macro_rules! decode_paired {
    // Base implementation for processing both pairs that handles the common logic
    (@impl $reader:expr, $out_r1:expr, $out_r2:expr, $write_fn:expr) => {{
        let mut ibuf = itoa::Buffer::new(); // index buffer
        let mut sbuffer = Vec::new(); // reusable buffer for decoding nucleotides (sequence)
        let mut xbuffer = Vec::new(); // reusable buffer for decoding nucleotides (extended)

        let mut num_records = 0;
        while let Some(pair) = $reader.next_paired() {
            // Catch any errors that occur during reading
            let pair = pair?;

            // Decode the nucleotides
            pair.decode_s(&mut sbuffer)?;
            pair.decode_x(&mut xbuffer)?;

            // Encode the index
            let index = ibuf.format(num_records).as_bytes();

            // Use the provided write function
            $write_fn(&mut $out_r1, &mut $out_r2, &index, &sbuffer, &xbuffer)?;

            num_records += 1;

            // Clear the buffers for the next record
            sbuffer.clear();
            xbuffer.clear();
        }

        // Finalize the writers
        $out_r1.flush()?;
        $out_r2.flush()?;

        Ok(())
    }};

    // FASTA format (without quality scores)
    ($reader:expr, $out_r1:expr, $out_r2:expr) => {{
        decode_paired!(@impl $reader, $out_r1, $out_r2, |out1, out2, index, seq1, seq2| {
            write_fasta_parts(out1, index, seq1)?;
            write_fasta_parts(out2, index, seq2)
        })
    }};

    // FASTQ format (with quality scores)
    ($reader:expr, $out_r1:expr, $out_r2:expr, $qual_r1:expr, $qual_r2:expr) => {{
        decode_paired!(@impl $reader, $out_r1, $out_r2, |out1, out2, index, seq1, seq2| {
            write_fastq_parts(out1, index, seq1, &$qual_r1)?;
            write_fastq_parts(out2, index, seq2, &$qual_r2)
        })
    }};
}

#[macro_export]
macro_rules! decode_paired_mate {
    // Base implementation for processing a single mate that handles the common logic
    (@impl $reader:expr, $out:expr, $next_fn:expr, $write_fn:expr) => {{
        let mut ibuf = itoa::Buffer::new(); // index buffer
        let mut buffer = Vec::new(); // reusable buffer for decoding nucleotides

        let mut num_records = 0;
        while let Some(record) = $next_fn(&mut $reader) {
            // Catch any errors that occur during reading
            let record = record?;

            // Decode the nucleotides
            record.decode(&mut buffer)?;

            // Encode the index
            let index = ibuf.format(num_records).as_bytes();

            // Use the provided write function
            $write_fn(&mut $out, &index, &buffer)?;

            num_records += 1;

            // Clear the buffers for the next record
            buffer.clear();
        }

        // Finalize the writers
        $out.flush()?;

        Ok(())
    }};

    // FASTA format (without quality scores)
    ($reader:expr, $out:expr, $next_fn:expr) => {{
        decode_paired_mate!(@impl $reader, $out, $next_fn, |out, index, seq| {
            write_fasta_parts(out, index, seq)
        })
    }};

    // FASTQ format (with quality scores)
    ($reader:expr, $out:expr, $next_fn:expr, $qual:expr) => {{
        decode_paired_mate!(@impl $reader, $out, $next_fn, |out, index, seq|
            write_fastq_parts(out, index, seq, &$qual)
        )
    }};
}

#[macro_export]
macro_rules! decode_single {
    // Base implementation that handles the common logic
    (@impl $reader:expr, $out:expr, $write_fn:expr) => {{
        let mut ibuf = itoa::Buffer::new(); // index buffer
        let mut sbuffer = Vec::new(); // reusable buffer for decoding nucleotides (sequence)

        let mut num_records = 0;
        while let Some(record) = $reader.next() {
            // Catch any errors that occur during reading
            let record = record?;

            // Decode the nucleotides
            record.decode(&mut sbuffer)?;

            // Encode the index
            let index = ibuf.format(num_records).as_bytes();

            // Use the provided write function
            $write_fn(&mut $out, &index, &sbuffer)?;

            num_records += 1;

            // Clear the buffers for the next record
            sbuffer.clear();
        }

        // Finalize the writer
        $out.flush()?;

        Ok(())
    }};

    // FASTA format (without quality scores)
    ($reader:expr, $out:expr) => {{
        decode_single!(@impl $reader, $out, |out, index, seq| write_fasta_parts(out, index, seq))
    }};

    // FASTQ format (with quality scores)
    ($reader:expr, $out:expr, $qual:expr) => {{
        decode_single!(@impl $reader, $out, |out, index, seq| write_fastq_parts(out, index, seq, &$qual))
    }};
}
