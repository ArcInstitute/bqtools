use std::io::Write;
use std::sync::Arc;

use anyhow::{bail, Result};
use binseq::{BinseqHeader, MmapReader, ParallelProcessor, RefRecord};
use parking_lot::Mutex;

mod utils;
use utils::{write_record, SplitWriter};

use crate::cli::{DecodeCommand, FileFormat, Mate};

pub type Writer = Box<dyn Write + Send>;

/// A struct for decoding BINSEQ data back to FASTQ format.
#[derive(Clone)]
pub struct Decoder {
    /// Local write buffers
    mixed: Vec<u8>, // General purpose, interleaved or singlets
    left: Vec<u8>, // Used when writing pairs of files (R1/R2)
    right: Vec<u8>,

    /// Local buffer for decoding primary
    sbuf: Vec<u8>,
    /// Local buffer for decoding secondary
    xbuf: Vec<u8>,
    /// Local count of records
    local_count: usize,
    /// Quality buffer
    quality: Vec<u8>,

    /// Options
    format: FileFormat,
    mate: Option<Mate>,
    is_split: bool,

    /// Global values
    global_writer: Arc<Mutex<SplitWriter>>,
    num_records: Arc<Mutex<usize>>,
}

impl Decoder {
    pub fn new(writer: SplitWriter, format: FileFormat, mate: Option<Mate>) -> Self {
        Decoder {
            mixed: Vec::new(),
            left: Vec::new(),
            right: Vec::new(),
            sbuf: Vec::new(),
            xbuf: Vec::new(),
            local_count: 0,
            quality: Vec::new(),
            format,
            mate,
            is_split: writer.is_split(),
            global_writer: Arc::new(Mutex::new(writer)),
            num_records: Arc::new(Mutex::new(0)),
        }
    }

    pub fn num_records(&self) -> usize {
        *self.num_records.lock()
    }
}
impl ParallelProcessor for Decoder {
    fn process_record(&mut self, record: RefRecord) -> Result<(), binseq::Error> {
        // clear decoding buffers
        self.sbuf.clear();
        self.xbuf.clear();

        // decode index
        let mut ibuf = itoa::Buffer::new();
        let index = ibuf.format(record.id()).as_bytes();

        // decode sequences
        record.decode_s(&mut self.sbuf)?;
        if self.quality.len() < self.sbuf.len() {
            self.quality.resize(self.sbuf.len(), b'?');
        }
        if record.paired() {
            record.decode_x(&mut self.xbuf)?;
            if self.quality.len() < self.xbuf.len() {
                self.quality.resize(self.xbuf.len(), b'?');
            }
        }

        match self.mate {
            Some(Mate::Both) => {
                if self.is_split {
                    write_record(
                        &mut self.left,
                        index,
                        &self.sbuf,
                        &self.quality,
                        self.format,
                    )?;
                    write_record(
                        &mut self.right,
                        index,
                        &self.xbuf,
                        &self.quality,
                        self.format,
                    )?;
                } else {
                    write_record(
                        &mut self.mixed,
                        index,
                        &self.sbuf,
                        &self.quality,
                        self.format,
                    )?;
                    write_record(
                        &mut self.mixed,
                        index,
                        &self.xbuf,
                        &self.quality,
                        self.format,
                    )?;
                }
            }
            Some(Mate::One) | None => {
                write_record(
                    &mut self.mixed,
                    index,
                    &self.sbuf,
                    &self.quality,
                    self.format,
                )?;
            }
            Some(Mate::Two) => {
                write_record(
                    &mut self.mixed,
                    index,
                    &self.xbuf,
                    &self.quality,
                    self.format,
                )?;
            }
        }

        self.local_count += 1;
        Ok(())
    }

    fn on_batch_complete(&mut self) -> Result<(), binseq::Error> {
        // Lock the mutex to write to the global buffer
        {
            let mut writer = self.global_writer.lock();
            if writer.is_split() {
                writer.write_split(&self.left, true)?;
                writer.write_split(&self.right, false)?;
            } else {
                writer.write_interleaved(&self.mixed)?;
            }
            writer.flush()?;
        }
        // Lock the mutex to update the number of records
        {
            let mut num_records = self.num_records.lock();
            *num_records += self.local_count;
        }

        // Clear the local buffer and reset the local record count
        self.mixed.clear();
        self.left.clear();
        self.right.clear();
        self.local_count = 0;
        Ok(())
    }
}

fn build_writer(args: &DecodeCommand, header: BinseqHeader) -> Result<SplitWriter> {
    let format = args.output.format()?;

    // Split writer
    if args.output.prefix.is_some() {
        if header.xlen == 0 {
            bail!("Cannot split file into two. No extended sequence channel");
        }
        match args.output.mate {
            Mate::Both => {
                let (r1, r2) = args.output.as_paired_writer(format)?;
                let split = SplitWriter::new_split(r1, r2);
                Ok(split)
            }
            _ => {
                eprintln!("Warning: Ignoring prefix as mate was provided");
                // Interleaved writer
                let writer = args.output.as_writer()?;
                let split = SplitWriter::new_interleaved(writer);
                Ok(split)
            }
        }
    } else {
        match args.output.mate {
            Mate::One | Mate::Two => {
                eprintln!("Warning: Ignoring mate as single channel in file");
            }
            _ => {}
        }
        // Interleaved writer
        let writer = args.output.as_writer()?;
        let split = SplitWriter::new_interleaved(writer);
        Ok(split)
    }
}

pub fn run(args: DecodeCommand) -> Result<()> {
    let reader = MmapReader::new(args.input.path())?;
    let writer = build_writer(&args, reader.header())?;

    let format = args.output.format()?;
    let mate = if reader.header().xlen > 0 {
        Some(args.output.mate())
    } else {
        None
    };
    let proc = Decoder::new(writer, format, mate);
    reader.process_parallel(proc.clone(), args.output.threads())?;

    eprintln!("Processed {} records...", proc.num_records());
    Ok(())
}
