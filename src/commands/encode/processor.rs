use std::io::Write;
use std::sync::Arc;

use anyhow::anyhow;
use binseq::{
    bq::{BinseqHeader, BinseqWriter, BinseqWriterBuilder},
    vbq::{VBinseqHeader, VBinseqWriter, VBinseqWriterBuilder},
    BinseqRecord, Policy,
};
use log::trace;
use paraseq::{prelude::*, ProcessError};
use parking_lot::Mutex;

/// Default capacity for the buffer used by the processor.
const DEFAULT_CAPACITY: usize = 128 * 1024;

/// Default debug interval for logging progress
const DEBUG_INTERVAL: usize = 1024;

pub struct BinseqProcessor<W: Write + Send> {
    /* Thread-local fields */
    /// Encoder for the current thread
    writer: BinseqWriter<Vec<u8>>,
    /// Number of records written by this thread
    record_count: usize,
    /// Number of records skipped by this thread
    skipped_count: usize,
    /* Global fields */
    /// Global writer for the entire process
    global_writer: Arc<Mutex<BinseqWriter<W>>>,
    /// Global counter for records written by all threads
    global_record_count: Arc<Mutex<usize>>,
    /// Global counter for records skipped by all threads
    global_skipped_count: Arc<Mutex<usize>>,
    /// Debug interval for logging progress
    debug_interval: Arc<Mutex<usize>>,
}

impl<W: Write + Send> BinseqProcessor<W> {
    pub fn new(header: BinseqHeader, policy: Policy, inner: W) -> binseq::Result<Self> {
        let local_inner = Vec::with_capacity(DEFAULT_CAPACITY);
        let writer = BinseqWriterBuilder::default()
            .header(header)
            .policy(policy)
            .headless(true)
            .build(local_inner)?;
        let global_writer = BinseqWriterBuilder::default()
            .header(header)
            .policy(policy)
            .build(inner)
            .map(|w| Arc::new(Mutex::new(w)))?;
        Ok(Self {
            writer,
            global_writer,
            record_count: 0,
            skipped_count: 0,
            global_record_count: Arc::new(Mutex::new(0)),
            global_skipped_count: Arc::new(Mutex::new(0)),
            debug_interval: Arc::new(Mutex::new(0)),
        })
    }

    /// Writes the current batch to the global writer.
    ///
    /// This function acquires a lock on the global writer and ingests the local buffer.
    fn write_batch(&mut self) -> binseq::Result<()> {
        // Aquire lock on global writer
        let mut global = self.global_writer.lock();

        // Ingestion clears the local buffer
        global.ingest(&mut self.writer)?;

        // Flush the global writer to avoid thread contention
        global.flush()
    }

    /// Updates global counters
    fn update_global_counters(&mut self) {
        *self.global_record_count.lock() += self.record_count;
        *self.global_skipped_count.lock() += self.skipped_count;
        *self.debug_interval.lock() += 1;
        self.record_count = 0;
        self.skipped_count = 0;
        if (*self.debug_interval.lock()).is_multiple_of(DEBUG_INTERVAL) {
            trace!("Processed {} records", self.global_record_count.lock());
        }
    }

    /// Get global number of records processed
    pub fn get_global_record_count(&self) -> usize {
        *self.global_record_count.lock()
    }

    /// Get global number of records skipped
    pub fn get_global_skipped_count(&self) -> usize {
        *self.global_skipped_count.lock()
    }
}
impl<W: Write + Send> Clone for BinseqProcessor<W> {
    fn clone(&self) -> Self {
        Self {
            writer: self.writer.clone(),
            global_writer: self.global_writer.clone(),
            record_count: self.record_count,
            skipped_count: self.skipped_count,
            global_record_count: self.global_record_count.clone(),
            global_skipped_count: self.global_skipped_count.clone(),
            debug_interval: self.debug_interval.clone(),
        }
    }
}

impl<W: Write + Send, Rf: paraseq::Record> ParallelProcessor<Rf> for BinseqProcessor<W> {
    fn process_record(&mut self, record: Rf) -> paraseq::Result<()> {
        if self
            .writer
            .write_record(None, &record.seq())
            .map_err(IntoProcessError::into_process_error)?
        {
            self.record_count += 1;
        } else {
            self.skipped_count += 1;
        }

        // implicitly skip the record if encoding fails
        Ok(())
    }

    fn on_batch_complete(&mut self) -> paraseq::Result<()> {
        self.update_global_counters();
        self.write_batch()
            .map_err(IntoProcessError::into_process_error)?;
        Ok(())
    }
}

impl<W: Write + Send, Rf: paraseq::Record> PairedParallelProcessor<Rf> for BinseqProcessor<W> {
    fn process_record_pair(&mut self, record1: Rf, record2: Rf) -> paraseq::Result<()> {
        if self
            .writer
            .write_paired_record(None, &record1.seq(), &record2.seq())
            .map_err(IntoProcessError::into_process_error)?
        {
            self.record_count += 1;
        } else {
            self.skipped_count += 1;
        }

        // implicitly skip the record if encoding fails
        Ok(())
    }

    fn on_batch_complete(&mut self) -> paraseq::Result<()> {
        self.update_global_counters();
        self.write_batch()
            .map_err(IntoProcessError::into_process_error)?;
        Ok(())
    }
}

pub struct VBinseqProcessor<W: Write + Send> {
    /* Thread-local fields */
    /// Encoder for the current thread
    writer: VBinseqWriter<Vec<u8>>,
    /// Number of records written by this thread
    record_count: usize,
    /// Number of records skipped by this thread
    skipped_count: usize,
    /// Buffer for decoding primary buffers (when recoding)
    sbuf: Vec<u8>,
    /// Buffer for decoding extended buffers (when recoding)
    xbuf: Vec<u8>,
    /// Buffer for decoding primary headers (when recoding)
    sheader: Vec<u8>,
    /// Buffer for decoding extended headers (when recoding)
    xheader: Vec<u8>,
    /* Global fields */
    /// Global writer for the entire process
    global_writer: Arc<Mutex<VBinseqWriter<W>>>,
    /// Global counter for records written by all threads
    global_record_count: Arc<Mutex<usize>>,
    /// Global counter for records skipped by all threads
    global_skipped_count: Arc<Mutex<usize>>,
    /// Debug interval for logging progress
    debug_interval: Arc<Mutex<usize>>,
}

impl<W: Write + Send> VBinseqProcessor<W> {
    pub fn new(header: VBinseqHeader, policy: Policy, inner: W) -> binseq::Result<Self> {
        let local_inner = Vec::with_capacity(DEFAULT_CAPACITY);
        let writer = VBinseqWriterBuilder::default()
            .header(header)
            .policy(policy)
            .headless(true)
            .build(local_inner)?;
        let global_writer = VBinseqWriterBuilder::default()
            .header(header)
            .policy(policy)
            .build(inner)
            .map(|w| Arc::new(Mutex::new(w)))?;
        Ok(Self {
            writer,
            global_writer,
            record_count: 0,
            skipped_count: 0,
            sbuf: Vec::default(),
            xbuf: Vec::default(),
            sheader: Vec::default(),
            xheader: Vec::default(),
            global_record_count: Arc::new(Mutex::new(0)),
            global_skipped_count: Arc::new(Mutex::new(0)),
            debug_interval: Arc::new(Mutex::new(0)),
        })
    }

    /// Writes the current batch to the global writer.
    ///
    /// This function acquires a lock on the global writer and ingests the local buffer.
    fn write_batch(&mut self) -> binseq::Result<()> {
        // Aquire lock on global writer
        let mut global = self.global_writer.lock();

        // Ingestion clears the local buffer
        global.ingest(&mut self.writer)?;

        Ok(())
    }

    /// Updates global counters
    fn update_global_counters(&mut self) {
        *self.global_record_count.lock() += self.record_count;
        *self.global_skipped_count.lock() += self.skipped_count;
        *self.debug_interval.lock() += 1;
        self.record_count = 0;
        self.skipped_count = 0;
        if (*self.debug_interval.lock()).is_multiple_of(DEBUG_INTERVAL) {
            trace!("Processed {} records", self.global_record_count.lock());
        }
    }

    /// Get global number of records processed
    pub fn get_global_record_count(&self) -> usize {
        *self.global_record_count.lock()
    }

    /// Get global number of records skipped
    pub fn get_global_skipped_count(&self) -> usize {
        *self.global_skipped_count.lock()
    }

    /// Finish the global writer
    pub fn finish(&self) -> binseq::Result<()> {
        self.global_writer.lock().finish()
    }
}
impl<W: Write + Send> Clone for VBinseqProcessor<W> {
    fn clone(&self) -> Self {
        Self {
            writer: self.writer.clone(),
            global_writer: self.global_writer.clone(),
            record_count: self.record_count,
            skipped_count: self.skipped_count,
            sbuf: self.sbuf.clone(),
            xbuf: self.xbuf.clone(),
            sheader: self.sheader.clone(),
            xheader: self.xheader.clone(),
            global_record_count: self.global_record_count.clone(),
            global_skipped_count: self.global_skipped_count.clone(),
            debug_interval: self.debug_interval.clone(),
        }
    }
}

impl<W: Write + Send, Rf: paraseq::Record> ParallelProcessor<Rf> for VBinseqProcessor<W> {
    fn process_record(&mut self, record: Rf) -> paraseq::Result<()> {
        if self.writer.is_paired() {
            return Err(ProcessError::from(anyhow!(
                "Provided VBinseq Configuration is expecting paired records."
            )));
        }

        let write_status = self
            .writer
            .write_record(None, Some(record.id()), &record.seq(), record.qual())
            .map_err(IntoProcessError::into_process_error)?;

        if write_status {
            self.record_count += 1;
        } else {
            self.skipped_count += 1;
        }

        // implicitly skip the record if encoding fails
        Ok(())
    }

    fn on_batch_complete(&mut self) -> paraseq::Result<()> {
        self.update_global_counters();
        self.write_batch()
            .map_err(IntoProcessError::into_process_error)?;
        Ok(())
    }
}

impl<W: Write + Send, Rf: paraseq::Record> PairedParallelProcessor<Rf> for VBinseqProcessor<W> {
    fn process_record_pair(&mut self, record1: Rf, record2: Rf) -> paraseq::Result<()> {
        if !self.writer.is_paired() {
            return Err(ProcessError::from(anyhow!(
                "Provided VBinseq Configuration does not expect paired records."
            )));
        }

        let write_status = self
            .writer
            .write_paired_record(
                None,
                Some(record1.id()),
                &record1.seq(),
                record1.qual(),
                Some(record2.id()),
                &record2.seq(),
                record2.qual(),
            )
            .map_err(IntoProcessError::into_process_error)?;

        if write_status {
            self.record_count += 1;
        } else {
            self.skipped_count += 1;
        }

        // implicitly skip the record if encoding fails
        Ok(())
    }

    fn on_batch_complete(&mut self) -> paraseq::Result<()> {
        self.update_global_counters();
        self.write_batch()
            .map_err(IntoProcessError::into_process_error)?;
        Ok(())
    }
}

impl<W: Write + Send> binseq::ParallelProcessor for VBinseqProcessor<W> {
    fn process_record<B: BinseqRecord>(&mut self, record: B) -> binseq::Result<()> {
        // clear buffers before processing each record
        self.sbuf.clear();
        self.xbuf.clear();
        self.sheader.clear();
        self.xheader.clear();

        let write_status = if record.is_paired() {
            record.decode_s(&mut self.sbuf)?;
            record.decode_x(&mut self.xbuf)?;
            record.sheader(&mut self.sheader);
            record.xheader(&mut self.xheader);

            self.writer.write_paired_record(
                record.flag(),
                Some(&self.sheader),
                &self.sbuf,
                Some(&record.squal()),
                Some(&self.xheader),
                &self.xbuf,
                Some(&record.xqual()),
            )
        } else {
            record.decode_s(&mut self.sbuf)?;
            record.sheader(&mut self.sheader);

            self.writer.write_record(
                record.flag(),
                Some(&self.sheader),
                &self.sbuf,
                Some(&record.squal()),
            )
        }?;

        if write_status {
            self.record_count += 1;
        } else {
            self.skipped_count += 1;
        }

        Ok(())
    }

    fn on_batch_complete(&mut self) -> binseq::Result<()> {
        self.update_global_counters();
        self.write_batch()?;
        Ok(())
    }
}
