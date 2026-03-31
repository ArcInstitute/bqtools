use anyhow::Result;
use binseq::{
    bq, cbq,
    vbq::{self, BlockIndex},
    BinseqReader,
};
use serde::Serialize;
use thousands::Separable;

use crate::cli::InfoCommand;

#[derive(Serialize)]
struct BqInfo {
    format: &'static str,
    version: u8,
    bitsize: u8,
    paired: bool,
    flags: bool,
    sequence_length: u32,
    extended_length: Option<u32>,
    num_records: usize,
}
impl BqInfo {
    fn new(reader: &bq::MmapReader, num_records: usize) -> Self {
        let header = reader.header();
        let bitsize: u8 = header.bits.into();
        Self {
            format: "BQ",
            version: header.format,
            bitsize,
            paired: header.xlen > 0,
            flags: header.flags,
            sequence_length: header.slen,
            extended_length: if header.xlen > 0 {
                Some(header.xlen)
            } else {
                None
            },
            num_records,
        }
    }

    fn tabular(&self) {
        println!("Format            : {}", self.format);
        println!("Version           : {}", self.version);
        println!("Bitsize           : {}", self.bitsize);
        println!("Paired            : {}", self.paired);
        println!("Flags             : {}", self.flags);
        println!("Sequence Length   : {}", self.sequence_length);
        if let Some(extended_length) = self.extended_length {
            println!("Extended Length   : {}", extended_length);
        }
        println!(
            "Number of records : {}",
            self.num_records.separate_with_underscores()
        );
    }

    fn json(&self) {
        let json = serde_json::to_string_pretty(self).unwrap();
        println!("{}", json);
    }
}

#[derive(Serialize)]
struct VbqInfo {
    format: &'static str,
    version: u8,
    bitsize: u8,
    paired: bool,
    quality: bool,
    headers: bool,
    flags: bool,
    block_size: String,
    n_blocks: usize,
    num_records: usize,
    #[serde(skip)]
    block_index: BlockIndex,
}

impl VbqInfo {
    fn new(reader: &vbq::MmapReader, num_records: usize) -> Result<Self> {
        let header = reader.header();
        let index = reader.load_index()?;
        let bitsize: u8 = header.bits.into();
        let block_size = pprint_block_size(header.block as f64);
        Ok(Self {
            format: "VBQ",
            version: header.format,
            bitsize,
            paired: header.paired,
            quality: header.qual,
            headers: header.headers,
            flags: header.flags,
            block_size,
            n_blocks: index.n_blocks(),
            num_records,
            block_index: index,
        })
    }

    fn tabular(&self) {
        println!("-------------------------------");
        println!("             File              ");
        println!("-------------------------------");
        println!("Format              : {}", self.format);
        println!("Version             : {}", self.version);
        println!("-------------------------------");
        println!("           Metadata            ");
        println!("-------------------------------");
        println!("Bits per Nucleotide : {}", self.bitsize);
        println!("Paired              : {}", self.paired);
        println!("Quality:            : {}", self.quality);
        println!("Headers:            : {}", self.headers);
        println!("Flags               : {}", self.flags);
        println!("-------------------------------");
        println!("          Compression          ");
        println!("-------------------------------");
        println!("Virtual Block Size  : {}", self.block_size);
        println!("-------------------------------");
        println!("            Data               ");
        println!("-------------------------------");
        println!("Number of blocks    : {}", self.n_blocks);
        println!(
            "Number of records   : {}",
            self.num_records.separate_with_underscores()
        );
    }

    fn json(&self) {
        println!("{}", serde_json::to_string_pretty(self).unwrap());
    }

    fn print_index(&self) {
        self.block_index.pprint();
    }
}

#[derive(Serialize)]
struct CbqInfo {
    format: &'static str,
    version: u8,
    paired: bool,
    quality: bool,
    headers: bool,
    flags: bool,
    compression_level: u64,
    block_size: String,
    mean_block_size: String,
    num_blocks: usize,
    num_records: usize,
    #[serde(skip)]
    index: cbq::Index,
}
impl CbqInfo {
    fn new(reader: &cbq::MmapReader, num_records: usize) -> Self {
        let header = reader.header();
        let index = reader.index().to_owned();
        let block_size = pprint_block_size(header.block_size as f64);
        let avg_block_size = pprint_block_size(reader.index().average_block_size());
        Self {
            format: "CBQ",
            version: header.version,
            paired: header.is_paired(),
            quality: header.has_qualities(),
            headers: header.has_headers(),
            flags: header.has_flags(),
            compression_level: header.compression_level,
            block_size,
            mean_block_size: avg_block_size,
            num_blocks: index.num_blocks(),
            num_records,
            index,
        }
    }

    fn json(&self) {
        println!("{}", serde_json::to_string_pretty(self).unwrap())
    }

    fn tabular(&self) {
        println!("-------------------------------");
        println!("             File              ");
        println!("-------------------------------");
        println!("Format              : CBQ");
        println!("Version             : {}", self.version);
        println!("-------------------------------");
        println!("           Metadata            ");
        println!("-------------------------------");
        println!("Paired              : {}", self.paired);
        println!("Quality:            : {}", self.quality);
        println!("Headers:            : {}", self.headers);
        println!("Flags               : {}", self.flags);
        println!("-------------------------------");
        println!("          Compression          ");
        println!("-------------------------------");
        println!("Compression Level   : {}", self.compression_level);
        println!("Virtual Block Size  : {}", self.block_size);
        println!("Mean Block Size     : {}", self.mean_block_size);
        println!("-------------------------------");
        println!("            Data               ");
        println!("-------------------------------");
        println!("Number of blocks    : {}", self.num_blocks);
        println!(
            "Number of records   : {}",
            self.num_records.separate_with_underscores()
        );
    }

    fn print_index(&self) {
        self.index.pprint();
    }
}

fn log_reader_bq(reader: &bq::MmapReader, num_records: usize, as_json: bool) {
    let info = BqInfo::new(reader, num_records);
    if as_json {
        info.json();
    } else {
        info.tabular();
    }
}

fn log_reader_vbq(
    reader: &vbq::MmapReader,
    num_records: usize,
    print_index: bool,
    as_json: bool,
) -> Result<()> {
    let info = VbqInfo::new(reader, num_records)?;
    if print_index {
        info.print_index();
    } else if as_json {
        info.json();
    } else {
        info.tabular();
    }
    Ok(())
}

fn log_reader_cbq(
    reader: &cbq::MmapReader,
    num_records: usize,
    print_index: bool,
    print_block_headers: bool,
    as_json: bool,
) -> Result<()> {
    if print_block_headers {
        for header in reader.iter_block_headers() {
            println!("{:?}", header?);
        }
        return Ok(());
    }

    let info = CbqInfo::new(reader, num_records);
    if print_index {
        info.print_index();
    } else if as_json {
        info.json();
    } else {
        info.tabular();
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

pub fn run(args: &InfoCommand) -> Result<()> {
    let reader = BinseqReader::new(args.input.path())?;
    let num_records = reader.num_records()?;
    if args.opts.num {
        println!("{num_records}");
    } else {
        match reader {
            BinseqReader::Bq(ref bq_reader) => {
                log_reader_bq(bq_reader, num_records, args.opts.json)
            }
            BinseqReader::Vbq(ref vbq_reader) => {
                log_reader_vbq(
                    vbq_reader,
                    num_records,
                    args.opts.show_index,
                    args.opts.json,
                )?;
            }
            BinseqReader::Cbq(ref cbq_reader) => {
                log_reader_cbq(
                    cbq_reader,
                    num_records,
                    args.opts.show_index,
                    args.opts.show_headers,
                    args.opts.json,
                )?;
            }
        }
    }
    Ok(())
}
