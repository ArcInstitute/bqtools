use anyhow::Result;
use binseq::{bq, vbq, BinseqReader};

use crate::cli::CountCommand;

fn log_reader_bq(reader: &bq::MmapReader, num_records: usize) {
    let header = reader.header();
    println!("Format Version    : {}", header.format);
    println!("Sequence Length   : {}", header.slen);
    if header.xlen > 0 {
        println!("Extended Length   : {}", header.xlen);
    }
    println!("Number of records : {num_records}");
}

fn log_reader_vbq(reader: &vbq::MmapReader, num_records: usize) {
    let header = reader.header();
    println!("Format Version    : {}", header.format);
    println!("Compression:      : {}", header.compressed);
    println!("Quality:          : {}", header.qual);
    println!("Number of records : {num_records}");
}

pub fn run(args: &CountCommand) -> Result<()> {
    let reader = BinseqReader::new(args.input.path())?;
    let num_records = reader.num_records()?;
    if args.opts.num {
        println!("{num_records}");
    } else {
        match reader {
            BinseqReader::Bq(ref bq_reader) => log_reader_bq(bq_reader, num_records),
            BinseqReader::Vbq(ref vbq_reader) => log_reader_vbq(vbq_reader, num_records),
        }
    }
    Ok(())
}
