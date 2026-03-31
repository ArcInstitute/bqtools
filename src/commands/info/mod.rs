use anyhow::Result;
use binseq::{
    bq, cbq,
    vbq::{self, BlockIndex},
    BinseqReader,
};
use log::warn;
use serde::Serialize;
use thousands::Separable;

use crate::cli::InfoCommand;

#[derive(Serialize)]
struct BqInfo {
    path: String,
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
    fn new(path: String, reader: &bq::MmapReader, num_records: usize) -> Self {
        let header = reader.header();
        let bitsize: u8 = header.bits.into();
        Self {
            path,
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
        println!("Path                : {}", self.path);
        println!("Format              : {}", self.format);
        println!("Version             : {}", self.version);
        println!("Bitsize             : {}", self.bitsize);
        println!("Paired              : {}", self.paired);
        println!("Flags               : {}", self.flags);
        println!("Sequence Length     : {}", self.sequence_length);
        if let Some(extended_length) = self.extended_length {
            println!("Extended Length     : {}", extended_length);
        }
        println!(
            "Number of records   : {}",
            self.num_records.separate_with_underscores()
        );
    }

    fn num_records(&self) {
        println!("{}\t{}", self.num_records, self.path);
    }
}

#[derive(Serialize)]
struct VbqInfo {
    path: String,
    format: &'static str,
    version: u8,
    bitsize: u8,
    paired: bool,
    quality: bool,
    headers: bool,
    flags: bool,
    block_size: u64,
    n_blocks: usize,
    num_records: usize,
    #[serde(skip)]
    block_index: BlockIndex,
}

impl VbqInfo {
    fn new(path: String, reader: &vbq::MmapReader, num_records: usize) -> Result<Self> {
        let header = reader.header();
        let index = reader.load_index()?;
        let bitsize: u8 = header.bits.into();
        Ok(Self {
            path,
            format: "VBQ",
            version: header.format,
            bitsize,
            paired: header.paired,
            quality: header.qual,
            headers: header.headers,
            flags: header.flags,
            block_size: header.block,
            n_blocks: index.n_blocks(),
            num_records,
            block_index: index,
        })
    }

    fn tabular(&self) {
        println!("-------------------------------");
        println!("             File              ");
        println!("-------------------------------");
        println!("Path                : {}", self.path);
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
        println!(
            "Virtual Block Size  : {}",
            pprint_block_size(self.block_size as f64)
        );
        println!("-------------------------------");
        println!("            Data               ");
        println!("-------------------------------");
        println!("Number of blocks    : {}", self.n_blocks);
        println!(
            "Number of records   : {}",
            self.num_records.separate_with_underscores()
        );
    }

    fn print_index(&self) {
        self.block_index.pprint();
    }

    fn num_records(&self) {
        println!("{}\t{}", self.num_records, self.path);
    }
}

#[derive(Serialize)]
struct CbqInfo {
    path: String,
    format: &'static str,
    version: u8,
    paired: bool,
    quality: bool,
    headers: bool,
    flags: bool,
    compression_level: u64,
    block_size: u64,
    mean_block_size: f64,
    num_blocks: usize,
    num_records: usize,
    #[serde(skip)]
    index: cbq::Index,
}
impl CbqInfo {
    fn new(path: String, reader: &cbq::MmapReader, num_records: usize) -> Self {
        let header = reader.header();
        let index = reader.index().to_owned();
        let avg_block_size = reader.index().average_block_size();
        Self {
            path,
            format: "CBQ",
            version: header.version,
            paired: header.is_paired(),
            quality: header.has_qualities(),
            headers: header.has_headers(),
            flags: header.has_flags(),
            compression_level: header.compression_level,
            block_size: header.block_size,
            mean_block_size: avg_block_size,
            num_blocks: index.num_blocks(),
            num_records,
            index,
        }
    }

    fn tabular(&self) {
        println!("-------------------------------");
        println!("             File              ");
        println!("-------------------------------");
        println!("Path                : {}", self.path);
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
        println!(
            "Virtual Block Size  : {}",
            pprint_block_size(self.block_size as f64)
        );
        println!(
            "Mean Block Size     : {}",
            pprint_block_size(self.mean_block_size)
        );
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

    fn num_records(&self) {
        println!("{}\t{}", self.num_records, self.path);
    }
}

#[derive(Serialize)]
#[serde(untagged)]
enum BinseqInfo {
    Bq(BqInfo),
    Vbq(VbqInfo),
    Cbq(CbqInfo),
}
impl BinseqInfo {
    pub fn from_path(path: &str) -> Result<Self> {
        let reader = BinseqReader::new(path)?;
        let num_records = reader.num_records()?;
        match reader {
            BinseqReader::Bq(bq_reader) => Ok(BinseqInfo::Bq(BqInfo::new(
                path.to_string(),
                &bq_reader,
                num_records,
            ))),
            BinseqReader::Vbq(vbq_reader) => Ok(BinseqInfo::Vbq(VbqInfo::new(
                path.to_string(),
                &vbq_reader,
                num_records,
            )?)),
            BinseqReader::Cbq(cbq_reader) => Ok(BinseqInfo::Cbq(CbqInfo::new(
                path.to_string(),
                &cbq_reader,
                num_records,
            ))),
        }
    }
    pub fn tabular(&self) {
        match self {
            BinseqInfo::Bq(bq) => bq.tabular(),
            BinseqInfo::Vbq(vbq) => vbq.tabular(),
            BinseqInfo::Cbq(cbq) => cbq.tabular(),
        }
    }

    pub fn num_records(&self) {
        match self {
            BinseqInfo::Bq(bq) => bq.num_records(),
            BinseqInfo::Vbq(vbq) => vbq.num_records(),
            BinseqInfo::Cbq(cbq) => cbq.num_records(),
        }
    }

    pub fn print_index(&self) {
        match self {
            BinseqInfo::Bq(bq) => {
                warn!("No index to print for BQ path: {}", bq.path)
            }
            BinseqInfo::Vbq(vbq) => vbq.print_index(),
            BinseqInfo::Cbq(cbq) => cbq.print_index(),
        }
    }
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
    // case for just CBQ with block headers
    if args.opts.show_headers {
        for path in args.input.iter() {
            let reader = match cbq::MmapReader::new(path.as_str()) {
                Ok(reader) => reader,
                Err(e) => {
                    warn!("Unable to read path: {} - {}", path, e);
                    continue;
                }
            };
            for header in reader.iter_block_headers() {
                println!("{:?}", header?);
            }
        }
        return Ok(());
    }

    // all other cases
    let all_info: Vec<BinseqInfo> = args
        .input
        .iter()
        .filter_map(|path| match BinseqInfo::from_path(path.as_str()) {
            Ok(info) => Some(info),
            Err(_) => {
                warn!("Unable to read path: {}", path);
                None
            }
        })
        .collect();
    if args.opts.json {
        println!("{}", serde_json::to_string_pretty(&all_info)?);
    } else if args.opts.num {
        for info in all_info {
            info.num_records();
        }
    } else if args.opts.show_index {
        for info in all_info {
            info.print_index();
        }
    } else {
        for info in all_info {
            info.tabular();
        }
    }
    Ok(())
}
