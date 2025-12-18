use anyhow::Result;
use binseq::{bq, vbq, BinseqReader};

use crate::cli::CountCommand;

fn log_reader_bq(reader: &bq::MmapReader, num_records: usize) {
    let header = reader.header();
    let bitsize: u8 = header.bits.into();
    println!("Format Version    : {}", header.format);
    println!("Bitsize           : {bitsize}");
    println!("Paired            : {}", header.xlen > 0);
    println!("Flags             : {}", header.flags);
    println!("Sequence Length   : {}", header.slen);
    if header.xlen > 0 {
        println!("Extended Length   : {}", header.xlen);
    }
    println!("Number of records : {num_records}");
}

fn log_reader_vbq(reader: &vbq::MmapReader, num_records: usize, print_index: bool) -> Result<()> {
    let header = reader.header();

    if print_index {
        let index = reader.load_index()?;
        index.pprint();
    } else {
        let bitsize: u8 = header.bits.into();
        println!("Format Version    : {}", header.format);
        println!("Bitsize           : {bitsize}");
        println!("Paired            : {}", header.paired);
        println!("Compression:      : {}", header.compressed);
        println!("Quality:          : {}", header.qual);
        println!("Headers:          : {}", header.headers);
        println!("Flags             : {}", header.flags);
        println!("Number of records : {num_records}");
    }
    Ok(())
}

pub fn run(args: &CountCommand) -> Result<()> {
    let reader = BinseqReader::new(args.input.path())?;
    let num_records = reader.num_records()?;
    if args.opts.num {
        println!("{num_records}");
    } else {
        match reader {
            BinseqReader::Bq(ref bq_reader) => log_reader_bq(bq_reader, num_records),
            BinseqReader::Vbq(ref vbq_reader) => {
                log_reader_vbq(vbq_reader, num_records, args.opts.show_index)?;
            }
            BinseqReader::Cbq(_) => {
                unimplemented!("Count is not implemented yet for CBQ")
            }
        }
    }
    Ok(())
}
