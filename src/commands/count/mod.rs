use std::io::Read;

use anyhow::{bail, Result};
use binseq::{BinseqRead, MmapReader, PairedMmapReader, PairedReader, SingleReader};

use crate::cli::CountCommand;

fn process_single_stream<R: Read>(mut reader: SingleReader<R>) -> Result<()> {
    let header = reader.header();
    println!("Format Version    : {}", header.format);
    println!("Sequence Length   : {}", header.slen);

    let mut num_records = 0;
    while let Some(record) = reader.next() {
        match record {
            Ok(_) => num_records += 1,
            Err(e) => bail!("Malformed input, record number: {} // {}", num_records, e),
        }
    }
    println!("Number of records : {}", num_records);
    Ok(())
}

fn process_single_mmap(path: &str, skip_val: bool) -> Result<()> {
    let mut reader = MmapReader::new(path)?;
    let header = reader.header();

    println!("Format Version    : {}", header.format);
    println!("Sequence Length   : {}", header.slen);

    let num_records = if skip_val {
        reader.num_records()
    } else {
        let mut num_records = 0;
        while let Some(record) = reader.next() {
            match record {
                Ok(_) => num_records += 1,
                Err(e) => bail!("Malformed input, record number: {} // {}", num_records, e),
            }
        }
        num_records
    };
    println!("Number of records : {}", num_records);
    Ok(())
}

fn process_paired_stream<R: Read>(mut reader: PairedReader<R>) -> Result<()> {
    let header = reader.header();
    println!("Format Version    : {}", header.format);
    println!("Sequence Length   : {}", header.slen);
    println!("Extended Length   : {}", header.xlen);

    let mut num_records = 0;
    while let Some(pair) = reader.next() {
        match pair {
            Ok(_) => num_records += 1,
            Err(e) => bail!(
                "Malformed input, record pair number: {} // {}",
                num_records,
                e
            ),
        }
    }
    println!("Number of records : {}", num_records);
    Ok(())
}

fn process_paired_mmap(path: &str, skip_val: bool) -> Result<()> {
    let mut reader = PairedMmapReader::new(path)?;
    let header = reader.header();

    println!("Format Version    : {}", header.format);
    println!("Sequence Length   : {}", header.slen);
    println!("Extended Length   : {}", header.xlen);

    let num_records = if skip_val {
        reader.num_records()
    } else {
        let mut num_records = 0;
        while let Some(pair) = reader.next() {
            match pair {
                Ok(_) => num_records += 1,
                Err(e) => bail!(
                    "Malformed input, record pair number: {} // {}",
                    num_records,
                    e
                ),
            }
        }
        num_records
    };

    println!("Number of records : {}", num_records);
    Ok(())
}

pub fn run(args: CountCommand) -> Result<()> {
    if args.input.decompress() {
        let in_handle = args.input.as_reader()?;
        match SingleReader::new(in_handle) {
            Ok(reader) => process_single_stream(reader),
            Err(_) => {
                let in_handle = args.input.as_reader()?;
                match PairedReader::new(in_handle) {
                    Ok(reader) => process_paired_stream(reader),
                    Err(e) => Err(e),
                }
            }
        }
    } else {
        let in_handle = args.input.as_reader()?;
        match SingleReader::new(in_handle) {
            Ok(_) => process_single_mmap(&args.input.input, args.skip_val),
            Err(_) => process_paired_mmap(&args.input.input, args.skip_val),
        }
    }
}
