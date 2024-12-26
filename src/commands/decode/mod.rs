mod utils;

use utils::{write_fasta_parts, write_fastq_parts};

use anyhow::Result;
use binseq::{BinseqRead, PairedRead, PairedReader, SingleReader};
use std::io::Read;

use crate::cli::{DecodeCommand, FileFormat};

fn process_single<R: Read>(args: DecodeCommand, reader: SingleReader<R>) -> Result<()> {
    match args.output.format()? {
        FileFormat::Fastq => process_single_fastq(args, reader),
        FileFormat::Fasta => process_single_fasta(args, reader),
    }
}

fn process_paired<R: Read>(args: DecodeCommand, reader: PairedReader<R>) -> Result<()> {
    match args.output.format()? {
        FileFormat::Fastq => process_paired_fastq(args, reader, FileFormat::Fastq),
        FileFormat::Fasta => process_paired_fasta(args, reader, FileFormat::Fasta),
    }
}

fn process_single_fastq<R: Read>(args: DecodeCommand, mut reader: SingleReader<R>) -> Result<()> {
    let mut out = args.output.as_writer()?;
    let header = reader.header();

    let qual = vec![b'?'; header.slen as usize]; // dummy quality values

    let mut ibuf = itoa::Buffer::new(); // index buffer
    let mut buffer = Vec::new(); // reusable buffer for decoding nucleotides

    let mut num_records = 0;
    while let Some(record) = reader.next() {
        // Catch any errors that occur during reading
        let record = record?;

        // Decode the nucleotides
        record.decode(&mut buffer)?;

        // Encode the index
        let index = ibuf.format(num_records).as_bytes();

        write_fastq_parts(&mut out, &index, &buffer, &qual)?;

        num_records += 1;

        // Clear the buffer for the next record
        buffer.clear();
    }

    // Finalize the writer
    out.flush()?;

    Ok(())
}

fn process_single_fasta<R: Read>(args: DecodeCommand, mut reader: SingleReader<R>) -> Result<()> {
    let mut out = args.output.as_writer()?;

    let mut ibuf = itoa::Buffer::new(); // index buffer
    let mut buffer = Vec::new(); // reusable buffer for decoding nucleotides

    let mut num_records = 0;
    while let Some(record) = reader.next() {
        // Catch any errors that occur during reading
        let record = record?;

        // Decode the nucleotides
        record.decode(&mut buffer)?;

        // Encode the index
        let index = ibuf.format(num_records).as_bytes();

        write_fasta_parts(&mut out, &index, &buffer)?;

        num_records += 1;

        // Clear the buffer for the next record
        buffer.clear();
    }

    // Finalize the writer
    out.flush()?;

    Ok(())
}

fn process_paired_fastq<R: Read>(
    args: DecodeCommand,
    mut reader: PairedReader<R>,
    format: FileFormat,
) -> Result<()> {
    let (mut out_r1, mut out_r2) = args.output.as_paired_writer(format)?;
    let header = reader.header();

    let qual_r1 = vec![b'?'; header.slen as usize]; // dummy quality values
    let qual_r2 = vec![b'?'; header.xlen as usize]; // dummy quality values

    let mut ibuf = itoa::Buffer::new(); // index buffer
    let mut sbuffer = Vec::new(); // reusable buffer for decoding nucleotides (sequence)
    let mut xbuffer = Vec::new(); // reusable buffer for decoding nucleotides (extended)

    let mut num_records = 0;
    while let Some(pair) = reader.next_paired() {
        // Catch any errors that occur during reading
        let pair = pair?;

        // Decode the nucleotides
        pair.decode_s(&mut sbuffer)?;
        pair.decode_x(&mut xbuffer)?;

        // Encode the index
        let index = ibuf.format(num_records).as_bytes();

        write_fastq_parts(&mut out_r1, &index, &sbuffer, &qual_r1)?;
        write_fastq_parts(&mut out_r2, &index, &xbuffer, &qual_r2)?;

        num_records += 1;

        // Clear the buffers for the next record
        sbuffer.clear();
        xbuffer.clear();
    }

    // Finalize the writers
    out_r1.flush()?;
    out_r2.flush()?;

    Ok(())
}

fn process_paired_fasta<R: Read>(
    args: DecodeCommand,
    mut reader: PairedReader<R>,
    format: FileFormat,
) -> Result<()> {
    let (mut out_r1, mut out_r2) = args.output.as_paired_writer(format)?;

    let mut ibuf = itoa::Buffer::new(); // index buffer
    let mut sbuffer = Vec::new(); // reusable buffer for decoding nucleotides (sequence)
    let mut xbuffer = Vec::new(); // reusable buffer for decoding nucleotides (extended)

    let mut num_records = 0;
    while let Some(pair) = reader.next_paired() {
        // Catch any errors that occur during reading
        let pair = pair?;

        // Decode the nucleotides
        pair.decode_s(&mut sbuffer)?;
        pair.decode_x(&mut xbuffer)?;

        // Encode the index
        let index = ibuf.format(num_records).as_bytes();

        write_fasta_parts(&mut out_r1, &index, &sbuffer)?;
        write_fasta_parts(&mut out_r2, &index, &xbuffer)?;

        num_records += 1;

        // Clear the buffers for the next record
        sbuffer.clear();
        xbuffer.clear();
    }

    // Finalize the writers
    out_r1.flush()?;
    out_r2.flush()?;

    Ok(())
}

pub fn run(args: DecodeCommand) -> Result<()> {
    let in_handle = args.input.as_reader()?;

    // Open the input BINSEQ
    match SingleReader::new(in_handle) {
        Ok(reader) => process_single(args, reader),
        Err(_) => {
            let in_handle = args.input.as_reader()?;
            match PairedReader::new(in_handle) {
                Ok(reader) => process_paired(args, reader),
                Err(e) => return Err(e.into()),
            }
        }
    }
}
