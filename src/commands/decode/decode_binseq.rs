use std::sync::Arc;

use binseq::prelude::*;
use binseq::Result;
use parking_lot::Mutex;

use super::{write_record_pair, SplitWriter};
use crate::cli::{FileFormat, Mate};

/// A struct for decoding BINSEQ data back to FASTQ format.
#[derive(Clone)]
pub struct Decoder {
    /// Local write buffers
    mixed: Vec<u8>, // General purpose, interleaved or singlets
    left: Vec<u8>, // Used when writing pairs of files (R1/R2)
    right: Vec<u8>,

    /// Local count of records
    local_count: usize,
    /// Quality buffer (primary)
    squal: Vec<u8>,
    /// Quality buffer (extended)
    xqual: Vec<u8>,

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
            local_count: 0,
            squal: Vec::new(),
            xqual: Vec::new(),
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
    fn process_record<B: BinseqRecord>(&mut self, record: B) -> Result<()> {
        let sbuf = record.sseq();
        let xbuf = record.xseq();

        // decode sequences
        let squal = if record.has_quality() {
            record.squal()
        } else {
            if self.squal.len() < sbuf.len() {
                self.squal.resize(sbuf.len(), b'?');
            }
            &self.squal
        };

        let xqual = if record.is_paired() {
            if record.has_quality() {
                record.xqual()
            } else {
                if self.xqual.len() < xbuf.len() {
                    self.xqual.resize(xbuf.len(), b'?');
                }
                &self.xqual
            }
        } else {
            if self.xqual.len() < xbuf.len() {
                self.xqual.resize(xbuf.len(), b'?');
            }
            &self.xqual
        };

        write_record_pair(
            &mut self.left,
            &mut self.right,
            &mut self.mixed,
            self.mate,
            self.is_split,
            sbuf,
            squal,
            &record.sheader(),
            xbuf,
            xqual,
            &record.xheader(),
            self.format,
        )?;

        self.local_count += 1;
        Ok(())
    }

    fn on_batch_complete(&mut self) -> Result<()> {
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
