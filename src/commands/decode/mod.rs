mod decode_macros;
mod utils;

use utils::{write_fasta_parts, write_fastq_parts};

use anyhow::Result;
use binseq::{BinseqRead, PairedRead, PairedReader, SingleReader};
use std::io::Read;

use crate::{
    cli::{DecodeCommand, FileFormat},
    decode_paired, decode_single,
};

fn process_single<R: Read>(args: DecodeCommand, mut reader: SingleReader<R>) -> Result<()> {
    let format = args.output.format()?;
    let mut out = args.output.as_writer()?;
    match format {
        FileFormat::Fastq => {
            let header = reader.header();
            let qual = vec![b'?'; header.slen as usize]; // dummy quality values
            decode_single!(reader, out, qual)
        }
        FileFormat::Fasta => decode_single!(reader, out),
    }
}

fn process_paired<R: Read>(args: DecodeCommand, mut reader: PairedReader<R>) -> Result<()> {
    let format = args.output.format()?;
    let (mut out_r1, mut out_r2) = args.output.as_paired_writer(format)?;

    match args.output.format()? {
        FileFormat::Fastq => {
            let header = reader.header();
            let qual_r1 = vec![b'?'; header.slen as usize]; // dummy quality values
            let qual_r2 = vec![b'?'; header.xlen as usize]; // dummy quality values
            decode_paired!(reader, out_r1, out_r2, qual_r1, qual_r2)
        }
        FileFormat::Fasta => {
            decode_paired!(reader, out_r1, out_r2)
        }
    }
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
