use std::io::Write;
use std::sync::Arc;

use anyhow::anyhow;
use binseq::{
    bq::{BinseqHeader, BinseqWriter, BinseqWriterBuilder},
    vbq::{VBinseqHeader, VBinseqWriter, VBinseqWriterBuilder},
    Policy,
};
use log::debug;
use paraseq::parallel::{
    InterleavedParallelProcessor, PairedParallelProcessor, ParallelProcessor, ProcessError,
};
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
        if *self.debug_interval.lock() % DEBUG_INTERVAL == 0 {
            debug!("Processed {} records", self.global_record_count.lock());
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

impl<W: Write + Send> ParallelProcessor for BinseqProcessor<W> {
    fn process_record<Rf: paraseq::Record>(&mut self, record: Rf) -> paraseq::parallel::Result<()> {
        if self
            .writer
            .write_nucleotides(0, &record.seq())
            .map_err(paraseq::parallel::IntoProcessError::into_process_error)?
        {
            self.record_count += 1;
        } else {
            self.skipped_count += 1;
        }

        // implicitly skip the record if encoding fails
        Ok(())
    }

    fn on_batch_complete(&mut self) -> paraseq::parallel::Result<()> {
        self.update_global_counters();
        self.write_batch()
            .map_err(paraseq::parallel::IntoProcessError::into_process_error)?;
        Ok(())
    }
}

impl<W: Write + Send> InterleavedParallelProcessor for BinseqProcessor<W> {
    fn process_interleaved_pair<Rf: paraseq::Record>(
        &mut self,
        record1: Rf,
        record2: Rf,
    ) -> paraseq::parallel::Result<()> {
        if self
            .writer
            .write_paired(0, &record1.seq(), &record2.seq())
            .map_err(paraseq::parallel::IntoProcessError::into_process_error)?
        {
            self.record_count += 1;
        } else {
            self.skipped_count += 1;
        }

        // implicitly skip the record if encoding fails
        Ok(())
    }

    fn on_batch_complete(&mut self) -> paraseq::parallel::Result<()> {
        self.update_global_counters();
        self.write_batch()
            .map_err(paraseq::parallel::IntoProcessError::into_process_error)?;
        Ok(())
    }
}

impl<W: Write + Send> PairedParallelProcessor for BinseqProcessor<W> {
    fn process_record_pair<Rf: paraseq::Record>(
        &mut self,
        record1: Rf,
        record2: Rf,
    ) -> paraseq::parallel::Result<()> {
        if self
            .writer
            .write_paired(0, &record1.seq(), &record2.seq())
            .map_err(paraseq::parallel::IntoProcessError::into_process_error)?
        {
            self.record_count += 1;
        } else {
            self.skipped_count += 1;
        }

        // implicitly skip the record if encoding fails
        Ok(())
    }

    fn on_batch_complete(&mut self) -> paraseq::parallel::Result<()> {
        self.update_global_counters();
        self.write_batch()
            .map_err(paraseq::parallel::IntoProcessError::into_process_error)?;
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
    /* Global fields */
    /// Global writer for the entire process
    global_writer: Arc<Mutex<VBinseqWriter<W>>>,
    /// Global counter for records written by all threads
    global_record_count: Arc<Mutex<usize>>,
    /// Global counter for records skipped by all threads
    global_skipped_count: Arc<Mutex<usize>>,
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
            global_record_count: Arc::new(Mutex::new(0)),
            global_skipped_count: Arc::new(Mutex::new(0)),
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
        self.record_count = 0;
        self.skipped_count = 0;
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
            global_record_count: self.global_record_count.clone(),
            global_skipped_count: self.global_skipped_count.clone(),
        }
    }
}

impl<W: Write + Send> ParallelProcessor for VBinseqProcessor<W> {
    fn process_record<Rf: paraseq::Record>(&mut self, record: Rf) -> paraseq::parallel::Result<()> {
        if self.writer.is_paired() {
            return Err(ProcessError::from(anyhow!(
                "Provided VBinseq Configuration is expecting paired records."
            )));
        }

        let write_status = if self.writer.has_quality() {
            self.writer
                .write_nucleotides_quality(0, &record.seq(), record.qual().unwrap())
        } else {
            self.writer.write_nucleotides(0, &record.seq())
        }
        .map_err(paraseq::parallel::IntoProcessError::into_process_error)?;

        if write_status {
            self.record_count += 1;
        } else {
            self.skipped_count += 1;
        }

        // implicitly skip the record if encoding fails
        Ok(())
    }

    fn on_batch_complete(&mut self) -> paraseq::parallel::Result<()> {
        self.update_global_counters();
        self.write_batch()
            .map_err(paraseq::parallel::IntoProcessError::into_process_error)?;
        Ok(())
    }
}

impl<W: Write + Send> InterleavedParallelProcessor for VBinseqProcessor<W> {
    fn process_interleaved_pair<Rf: paraseq::Record>(
        &mut self,
        record1: Rf,
        record2: Rf,
    ) -> paraseq::parallel::Result<()> {
        if !self.writer.is_paired() {
            return Err(ProcessError::from(anyhow!(
                "Provided VBinseq Configuration does not expect paired records."
            )));
        }

        let write_status = if self.writer.has_quality() {
            self.writer.write_nucleotides_quality_paired(
                0,
                &record1.seq(),
                &record2.seq(),
                record1.qual().unwrap(),
                record2.qual().unwrap(),
            )
        } else {
            self.writer
                .write_nucleotides_paired(0, &record1.seq(), &record2.seq())
        }
        .map_err(paraseq::parallel::IntoProcessError::into_process_error)?;

        if write_status {
            self.record_count += 1;
        } else {
            self.skipped_count += 1;
        }

        // implicitly skip the record if encoding fails
        Ok(())
    }

    fn on_batch_complete(&mut self) -> paraseq::parallel::Result<()> {
        self.update_global_counters();
        self.write_batch()
            .map_err(paraseq::parallel::IntoProcessError::into_process_error)?;
        Ok(())
    }
}

impl<W: Write + Send> PairedParallelProcessor for VBinseqProcessor<W> {
    fn process_record_pair<Rf: paraseq::Record>(
        &mut self,
        record1: Rf,
        record2: Rf,
    ) -> paraseq::parallel::Result<()> {
        if !self.writer.is_paired() {
            return Err(ProcessError::from(anyhow!(
                "Provided VBinseq Configuration does not expect paired records."
            )));
        }

        let write_status = if self.writer.has_quality() {
            self.writer.write_nucleotides_quality_paired(
                0,
                &record1.seq(),
                &record2.seq(),
                record1.qual().unwrap(),
                record2.qual().unwrap(),
            )
        } else {
            self.writer
                .write_nucleotides_paired(0, &record1.seq(), &record2.seq())
        }
        .map_err(paraseq::parallel::IntoProcessError::into_process_error)?;

        if write_status {
            self.record_count += 1;
        } else {
            self.skipped_count += 1;
        }

        // implicitly skip the record if encoding fails
        Ok(())
    }

    fn on_batch_complete(&mut self) -> paraseq::parallel::Result<()> {
        self.update_global_counters();
        self.write_batch()
            .map_err(paraseq::parallel::IntoProcessError::into_process_error)?;
        Ok(())
    }
}
