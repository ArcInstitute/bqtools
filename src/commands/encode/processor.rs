use std::io::Write;
use std::sync::Arc;

use anyhow::anyhow;
use binseq::{
    bq::{BinseqHeader, BinseqWriter, BinseqWriterBuilder},
    vbq::{VBinseqHeader, VBinseqWriter, VBinseqWriterBuilder},
    Policy,
};
use paraseq::parallel::{
    InterleavedParallelProcessor, IntoProcessError, PairedParallelProcessor, ParallelProcessor,
    ProcessError,
};
use parking_lot::Mutex;

use super::utils::{pad_sequence, truncate_sequence};
use crate::cli::{PadConfig, TruncateConfig};

/// Default capacity for the buffer used by the processor.
const DEFAULT_CAPACITY: usize = 128 * 1024;

pub struct BinseqProcessor<W: Write + Send> {
    /* Thread-local fields */
    /// Encoder for the current thread
    writer: BinseqWriter<Vec<u8>>,
    /// Truncation mode
    truncate: Option<TruncateConfig>,
    /// Padding mode
    padding: Option<PadConfig>,
    /// Number of records written by this thread
    record_count: usize,
    /// Number of records skipped by this thread
    skipped_count: usize,
    /// Padding vector for the current thread (only used when padding is enabled)
    smod: Vec<u8>,
    xmod: Vec<u8>,
    /// Header for the binseq writers
    header: BinseqHeader,
    /* Global fields */
    /// Global writer for the entire process
    global_writer: Arc<Mutex<BinseqWriter<W>>>,
    /// Global counter for records written by all threads
    global_record_count: Arc<Mutex<usize>>,
    /// Global counter for records skipped by all threads
    global_skipped_count: Arc<Mutex<usize>>,
}

impl<W: Write + Send> BinseqProcessor<W> {
    pub fn new(
        header: BinseqHeader,
        policy: Policy,
        truncate: Option<TruncateConfig>,
        padding: Option<PadConfig>,
        inner: W,
    ) -> binseq::Result<Self> {
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
            header,
            writer,
            global_writer,
            truncate,
            padding,
            record_count: 0,
            skipped_count: 0,
            smod: Vec::default(),
            xmod: Vec::default(),
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

        // Flush the global writer to avoid thread contention
        global.flush()
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
}
impl<W: Write + Send> Clone for BinseqProcessor<W> {
    fn clone(&self) -> Self {
        Self {
            writer: self.writer.clone(),
            header: self.header.clone(),
            truncate: self.truncate,
            padding: self.padding,
            smod: self.smod.clone(),
            xmod: self.xmod.clone(),
            global_writer: self.global_writer.clone(),
            record_count: self.record_count,
            skipped_count: self.skipped_count,
            global_record_count: self.global_record_count.clone(),
            global_skipped_count: self.global_skipped_count.clone(),
        }
    }
}

impl<W: Write + Send> ParallelProcessor for BinseqProcessor<W> {
    fn process_record<Rf: paraseq::Record>(&mut self, record: Rf) -> paraseq::parallel::Result<()> {
        // Pull the reference sequence from the record
        let ref_seq = &record.seq();

        // Apply sequence transformations (if required by config)
        let seq = {
            let trunc_seq = truncate_sequence(ref_seq, true, self.truncate);
            pad_sequence(&mut self.smod, trunc_seq, true, self.padding, self.header);
            if self.smod.is_empty() {
                trunc_seq
            } else {
                &self.smod
            }
        };

        // Encode and write the sequence
        if self
            .writer
            .write_nucleotides(0, seq)
            .map_err(|e| e.into_process_error())?
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
        self.write_batch().map_err(|e| e.into_process_error())?;
        Ok(())
    }
}

impl<W: Write + Send> InterleavedParallelProcessor for BinseqProcessor<W> {
    fn process_interleaved_pair<Rf: paraseq::Record>(
        &mut self,
        record1: Rf,
        record2: Rf,
    ) -> paraseq::parallel::Result<()> {
        // Pull the sequence data from the records
        let s_ref_seq = &record1.seq();
        let x_ref_seq = &record2.seq();

        // Apply sequence transformations to primary
        let s_seq = {
            let trunc_seq = truncate_sequence(s_ref_seq, true, self.truncate);
            pad_sequence(&mut self.smod, trunc_seq, true, self.padding, self.header);
            if self.smod.is_empty() {
                trunc_seq
            } else {
                &self.smod
            }
        };

        // Apply sequence transformations to extended
        let x_seq = {
            let trunc_seq = truncate_sequence(x_ref_seq, false, self.truncate);
            pad_sequence(&mut self.xmod, trunc_seq, false, self.padding, self.header);
            if self.xmod.is_empty() {
                trunc_seq
            } else {
                &self.xmod
            }
        };

        if self
            .writer
            .write_paired(0, s_seq, x_seq)
            .map_err(|e| e.into_process_error())?
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
        self.write_batch().map_err(|e| e.into_process_error())?;
        Ok(())
    }
}

impl<W: Write + Send> PairedParallelProcessor for BinseqProcessor<W> {
    fn process_record_pair<Rf: paraseq::Record>(
        &mut self,
        record1: Rf,
        record2: Rf,
    ) -> paraseq::parallel::Result<()> {
        // Pull the sequence data from the records
        let s_ref_seq = &record1.seq();
        let x_ref_seq = &record2.seq();

        // Apply sequence transformations to primary
        let s_seq = {
            let trunc_seq = truncate_sequence(s_ref_seq, true, self.truncate);
            pad_sequence(&mut self.smod, trunc_seq, true, self.padding, self.header);
            if self.smod.is_empty() {
                trunc_seq
            } else {
                &self.smod
            }
        };

        // Apply sequence transformations to extended
        let x_seq = {
            let trunc_seq = truncate_sequence(x_ref_seq, false, self.truncate);
            pad_sequence(&mut self.xmod, trunc_seq, false, self.padding, self.header);
            if self.xmod.is_empty() {
                trunc_seq
            } else {
                &self.xmod
            }
        };

        if self
            .writer
            .write_paired(0, s_seq, x_seq)
            .map_err(|e| e.into_process_error())?
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
        self.write_batch().map_err(|e| e.into_process_error())?;
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
        .map_err(|e| e.into_process_error())?;

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
        self.write_batch().map_err(|e| e.into_process_error())?;
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
        .map_err(|e| e.into_process_error())?;

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
        self.write_batch().map_err(|e| e.into_process_error())?;
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
        .map_err(|e| e.into_process_error())?;

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
        self.write_batch().map_err(|e| e.into_process_error())?;
        Ok(())
    }
}
