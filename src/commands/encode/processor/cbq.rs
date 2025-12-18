use std::io::Write;
use std::sync::Arc;

use binseq::cbq::{ColumnarBlockWriter, FileHeader, SequencingRecordBuilder};
use log::trace;
use paraseq::prelude::*;
use parking_lot::Mutex;

use super::{DEBUG_INTERVAL, DEFAULT_CAPACITY};

pub struct CbqEncoder<W: Write + Send> {
    /* Thread-local fields */
    /// Encoder for the current thread
    writer: ColumnarBlockWriter<Vec<u8>>,
    /// Number of records written by this thread
    record_count: usize,
    /* Global fields */
    /// Global writer for the entire process
    global_writer: Arc<Mutex<ColumnarBlockWriter<W>>>,
    /// Global counter for records written by all threads
    global_record_count: Arc<Mutex<usize>>,
    /// Debug interval for logging progress
    debug_interval: Arc<Mutex<usize>>,
}

impl<W: Write + Send> Clone for CbqEncoder<W> {
    fn clone(&self) -> Self {
        Self {
            writer: self.writer.clone(),
            record_count: self.record_count,
            global_writer: self.global_writer.clone(),
            global_record_count: self.global_record_count.clone(),
            debug_interval: self.debug_interval.clone(),
        }
    }
}

impl<W: Write + Send> CbqEncoder<W> {
    pub fn new(header: FileHeader, inner: W) -> binseq::Result<Self> {
        let writer =
            ColumnarBlockWriter::new_headless(Vec::with_capacity(DEFAULT_CAPACITY), header);
        let global_writer =
            ColumnarBlockWriter::new(inner, header).map(|w| Arc::new(Mutex::new(w)))?;
        Ok(Self {
            writer,
            global_writer,
            record_count: 0,
            global_record_count: Arc::new(Mutex::new(0)),
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
        *self.debug_interval.lock() += 1;
        self.record_count = 0;
        if (*self.debug_interval.lock()).is_multiple_of(DEBUG_INTERVAL) {
            trace!("Processed {} records", self.global_record_count.lock());
        }
    }

    /// Get global number of records processed
    pub fn get_global_record_count(&self) -> usize {
        *self.global_record_count.lock()
    }

    /// Get global number of records skipped
    ///
    /// CBQ does not skip records
    pub fn get_global_skipped_count(&self) -> usize {
        0
    }

    fn has_headers(&self) -> bool {
        self.writer.header().has_headers()
    }

    /// Finish the global writer
    pub fn finish(&self) -> anyhow::Result<()> {
        self.global_writer.lock().finish()
    }
}

impl<W: Write + Send, Rf: paraseq::Record> ParallelProcessor<Rf> for CbqEncoder<W> {
    fn process_record(&mut self, record: Rf) -> paraseq::Result<()> {
        let seq = &record.seq();
        let rec = SequencingRecordBuilder::default()
            .s_seq(seq)
            .opt_s_qual(record.qual())
            .opt_s_header(self.has_headers().then(|| record.id()))
            .build()?;

        self.writer.push(rec)?;
        self.record_count += 1;

        Ok(())
    }

    fn on_batch_complete(&mut self) -> paraseq::Result<()> {
        self.update_global_counters();
        self.write_batch()
            .map_err(IntoProcessError::into_process_error)?;
        Ok(())
    }
}

impl<W: Write + Send, Rf: paraseq::Record> PairedParallelProcessor<Rf> for CbqEncoder<W> {
    fn process_record_pair(&mut self, r1: Rf, r2: Rf) -> paraseq::Result<()> {
        let s_seq = &r1.seq();
        let x_seq = &r2.seq();

        let rec = SequencingRecordBuilder::default()
            .s_seq(s_seq)
            .x_seq(x_seq)
            .opt_s_qual(r1.qual())
            .opt_x_qual(r2.qual())
            .opt_s_header(self.has_headers().then(|| r1.id()))
            .opt_x_header(self.has_headers().then(|| r2.id()))
            .build()?;

        self.writer.push(rec)?;
        self.record_count += 1;

        Ok(())
    }

    fn on_batch_complete(&mut self) -> paraseq::Result<()> {
        self.update_global_counters();
        self.write_batch()
            .map_err(IntoProcessError::into_process_error)?;
        Ok(())
    }
}
