use anyhow::Result;
use binseq::{BinseqRead, SingleReader};
use std::io::Write;

use crate::cli::FastqExport;

fn write_fastq_parts<W: Write>(
    writer: &mut W,
    index: &[u8],
    sequence: &[u8],
    quality: &[u8],
) -> Result<()> {
    writer.write_all(b"@seq.")?;
    writer.write_all(index)?;
    writer.write_all(b"\n")?;
    writer.write_all(sequence)?;
    writer.write_all(b"\n+\n")?;
    writer.write_all(quality)?;
    writer.write_all(b"\n")?;
    Ok(())
}

pub fn run(args: FastqExport) -> Result<()> {
    let in_handle = args.input.as_reader()?;
    let mut out_handle = args.output.as_writer()?;

    // Open the input BINSEQ
    let mut reader = SingleReader::new(in_handle)?;
    let header = reader.header();
    let mut num_records = 0;
    let qual = vec![b'?'; header.slen as usize]; // dummy quality values

    let mut ibuf = itoa::Buffer::new();
    let mut buffer = Vec::new(); // reusable buffer for decoding nucleotides
    while let Some(record) = reader.next() {
        // Catch any errors that occur during reading
        let record = record?;

        // Decode the nucleotides
        record.decode(&mut buffer)?;

        // Encode the index
        let index = ibuf.format(num_records).as_bytes();

        write_fastq_parts(&mut out_handle, &index, &buffer, &qual)?;

        num_records += 1;

        // Clear the buffer for the next record
        buffer.clear();
    }

    // Finalize the writer
    out_handle.flush()?;

    Ok(())
}
