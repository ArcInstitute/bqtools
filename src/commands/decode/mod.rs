mod decode_macros;
mod utils;

use utils::{write_fasta_parts, write_fastq_parts};

use anyhow::Result;
use binseq::{BinseqRead, PairedRead, PairedReader, SingleReader};
use std::io::Read;

use crate::{
    cli::{DecodeCommand, FileFormat, Mate},
    decode_paired, decode_paired_mate, decode_single,
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

    match args.output.mate() {
        Mate::Both => {
            let (mut out_r1, mut out_r2) = args.output.as_paired_writer(format)?;
            match format {
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
        m => {
            let mut out = args.output.as_writer()?;
            match format {
                FileFormat::Fastq => {
                    let header = reader.header();
                    match m {
                        Mate::One => {
                            let qual = vec![b'?'; header.slen as usize];
                            decode_paired_mate!(reader, out, PairedRead::next_primary, qual)
                        }
                        Mate::Two => {
                            let qual = vec![b'?'; header.xlen as usize];
                            decode_paired_mate!(reader, out, PairedRead::next_extended, qual)
                        }
                        _ => unreachable!(),
                    }
                }
                FileFormat::Fasta => match m {
                    Mate::One => decode_paired_mate!(reader, out, PairedRead::next_primary),
                    Mate::Two => decode_paired_mate!(reader, out, PairedRead::next_extended),
                    _ => unreachable!(),
                },
            }
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
                Err(e) => Err(e),
            }
        }
    }
}
