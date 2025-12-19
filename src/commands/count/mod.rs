use anyhow::Result;
use binseq::{bq, cbq, vbq, BinseqReader};

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
    let index = reader.load_index()?;

    if print_index {
        index.pprint();
    } else {
        let bitsize: u8 = header.bits.into();
        let block_size = pprint_block_size(header.block as f64);
        println!("-------------------------------");
        println!("             File              ");
        println!("-------------------------------");
        println!("Format              : VBQ");
        println!("Version             : {}", header.format);
        println!("-------------------------------");
        println!("           Metadata            ");
        println!("-------------------------------");
        println!("Bits per Nucleotide : {}", bitsize);
        println!("Paired              : {}", header.is_paired());
        println!("Quality:            : {}", header.qual);
        println!("Headers:            : {}", header.headers);
        println!("Flags               : {}", header.flags);
        println!("-------------------------------");
        println!("          Compression          ");
        println!("-------------------------------");
        println!("Virtual Block Size  : {}", block_size);
        println!("-------------------------------");
        println!("            Data               ");
        println!("-------------------------------");
        println!("Number of blocks    : {}", index.n_blocks());
        println!("Number of records   : {num_records}");
    }
    Ok(())
}

fn log_reader_cbq(reader: &cbq::MmapReader, num_records: usize, print_index: bool) -> Result<()> {
    let header = reader.header();
    if print_index {
        let index = reader.index();
        index.pprint();
    } else {
        let block_size = pprint_block_size(header.block_size as f64);
        let avg_block_size = pprint_block_size(reader.index().average_block_size());
        println!("-------------------------------");
        println!("             File              ");
        println!("-------------------------------");
        println!("Format              : CBQ");
        println!("Version             : {}", header.version);
        println!("-------------------------------");
        println!("           Metadata            ");
        println!("-------------------------------");
        println!("Paired              : {}", header.is_paired());
        println!("Quality:            : {}", header.has_qualities());
        println!("Headers:            : {}", header.has_headers());
        println!("Flags               : {}", header.has_flags());
        println!("-------------------------------");
        println!("          Compression          ");
        println!("-------------------------------");
        println!("Compression Level   : {}", header.compression_level);
        println!("Virtual Block Size  : {block_size}");
        println!("Mean Block Size     : {avg_block_size}");
        println!("-------------------------------");
        println!("            Data               ");
        println!("-------------------------------");
        println!("Number of blocks    : {}", reader.num_blocks());
        println!("Number of records   : {num_records}");
    }
    Ok(())
}

fn pprint_block_size<T>(block_size: T) -> String
where
    T: Into<f64> + Copy,
{
    const KB: f64 = 1024.0;
    const MB: f64 = KB * KB;
    const GB: f64 = MB * KB;

    let block_size = block_size.into();
    if block_size < KB {
        format!("{block_size} bytes")
    } else if block_size < MB {
        format!("{:.2} KB", block_size / KB)
    } else if block_size < GB {
        format!("{:.2} MB", block_size / MB)
    } else {
        format!("{:.2} GB", block_size / GB)
    }
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
            BinseqReader::Cbq(ref cbq_reader) => {
                log_reader_cbq(cbq_reader, num_records, args.opts.show_index)?;
            }
        }
    }
    Ok(())
}
