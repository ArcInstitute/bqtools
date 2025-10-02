use std::sync::Arc;

use crate::cli::{FileFormat, Mate, SampleCommand};
use anyhow::Result;
use binseq::prelude::*;
use parking_lot::Mutex;
use rand::{Rng, SeedableRng};

use super::decode::{build_writer, write_record_pair, SplitWriter};

#[derive(Clone)]
struct SampleProcessor {
    /// Sampling Options
    fraction: f64,
    rng: rand::rngs::SmallRng,

    /// Local write buffers
    mixed: Vec<u8>, // General purpose, interleaved or singlets
    left: Vec<u8>, // Used when writing pairs of files (R1/R2)
    right: Vec<u8>,

    /// Local decoding buffers
    sbuf: Vec<u8>,
    xbuf: Vec<u8>,

    /// Quality buffers
    squal: Vec<u8>,
    xqual: Vec<u8>,

    /// Header buffers
    sheader: Vec<u8>,
    xheader: Vec<u8>,

    /// Write Options
    format: FileFormat,
    mate: Option<Mate>,
    is_split: bool,

    /// Global values
    global_writer: Arc<Mutex<SplitWriter>>,
}
impl SampleProcessor {
    pub fn new(
        fraction: f64,
        seed: u64,
        writer: SplitWriter,
        format: FileFormat,
        mate: Option<Mate>,
    ) -> Self {
        Self {
            fraction,
            format,
            mate,
            rng: rand::rngs::SmallRng::seed_from_u64(seed),
            mixed: Vec::new(),
            left: Vec::new(),
            right: Vec::new(),
            sbuf: Vec::new(),
            xbuf: Vec::new(),
            squal: Vec::new(),
            xqual: Vec::new(),
            sheader: Vec::new(),
            xheader: Vec::new(),
            is_split: writer.is_split(),
            global_writer: Arc::new(Mutex::new(writer)),
        }
    }
    pub fn clear_buffers(&mut self) {
        self.sbuf.clear();
        self.xbuf.clear();
    }
    pub fn include_record(&mut self) -> bool {
        self.rng.random_bool(self.fraction)
    }
}
impl ParallelProcessor for SampleProcessor {
    fn process_record<B: BinseqRecord>(&mut self, record: B) -> binseq::Result<()> {
        self.clear_buffers();

        // Decode sequences
        record.decode_s(&mut self.sbuf)?;
        record.sheader(&mut self.sheader);
        if record.is_paired() {
            record.decode_x(&mut self.xbuf)?;
            record.xheader(&mut self.xheader);
        }

        if self.include_record() {
            let squal = if record.has_quality() {
                record.squal()
            } else {
                if self.squal.len() < self.sbuf.len() {
                    self.squal.resize(self.sbuf.len(), b'?');
                }
                &self.squal
            };

            let xqual = if record.is_paired() {
                if record.has_quality() {
                    record.xqual()
                } else {
                    if self.xqual.len() < self.xbuf.len() {
                        self.xqual.resize(self.xbuf.len(), b'?');
                    }
                    &self.xqual
                }
            } else {
                if self.xqual.len() < self.xbuf.len() {
                    self.xqual.resize(self.xbuf.len(), b'?');
                }
                &self.xqual
            };

            write_record_pair(
                &mut self.left,
                &mut self.right,
                &mut self.mixed,
                self.mate,
                self.is_split,
                &self.sbuf,
                squal,
                &self.sheader,
                &self.xbuf,
                xqual,
                &self.xheader,
                self.format,
            )?;
        }

        Ok(())
    }

    fn on_batch_complete(&mut self) -> binseq::Result<()> {
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

        // Clear the local buffer and reset the local record count
        self.mixed.clear();
        self.left.clear();
        self.right.clear();
        Ok(())
    }
}

pub fn run(args: &SampleCommand) -> Result<()> {
    args.sample.validate()?;
    let reader = BinseqReader::new(args.input.path())?;
    let writer = build_writer(&args.output, reader.is_paired())?;
    let format = args.output.format()?;
    let mate = if reader.is_paired() {
        Some(args.output.mate())
    } else {
        None
    };
    let proc = SampleProcessor::new(args.sample.fraction, args.sample.seed, writer, format, mate);
    reader.process_parallel(proc.clone(), args.output.threads())?;
    Ok(())
}
