use std::{io::Write, ops::AddAssign, sync::Arc};

use binseq::{BinseqWriter, SequencingRecordBuilder};
use log::trace;
use paraseq::prelude::{IntoProcessError, PairedParallelProcessor, ParallelProcessor};
use parking_lot::Mutex;

/// Default debug interval for logging progress (batches)
const DEBUG_INTERVAL: usize = 1024;

pub struct Encoder<W: Write + Send> {
    /// Thread-local writer for the encoder.
    t_writer: BinseqWriter<Vec<u8>>,
    /// Thread-local record count for the encoder.
    t_count: usize,
    /// Thread-local skip count for the encoder.
    t_skip: usize,

    /// Global writer for the encoder.
    writer: Arc<Mutex<BinseqWriter<W>>>,
    /// Global record count for the encoder.
    count: Arc<Mutex<usize>>,
    /// Global skip count for the encoder.
    skip: Arc<Mutex<usize>>,
    /// Debug interval for logging progress
    debug_interval: Arc<Mutex<usize>>,
}
impl<W: Write + Send> Clone for Encoder<W> {
    fn clone(&self) -> Self {
        Self {
            t_writer: self.t_writer.clone(),
            t_count: self.t_count.clone(),
            t_skip: self.t_skip.clone(),
            writer: self.writer.clone(),
            count: self.count.clone(),
            skip: self.skip.clone(),
            debug_interval: self.debug_interval.clone(),
        }
    }
}
impl<W: Write + Send> Encoder<W> {
    pub fn new(writer: BinseqWriter<W>) -> binseq::Result<Self> {
        let t_writer = writer.new_headless_buffer()?;
        Ok(Self {
            writer: Arc::new(Mutex::new(writer)),
            t_writer,
            t_count: 0,
            t_skip: 0,
            count: Arc::new(Mutex::new(0)),
            skip: Arc::new(Mutex::new(0)),
            debug_interval: Arc::new(Mutex::new(DEBUG_INTERVAL)),
        })
    }

    fn write_batch(&mut self) -> binseq::Result<()> {
        self.writer.lock().ingest(&mut self.t_writer)
    }

    fn update_global_counters(&mut self) {
        // update counts
        {
            self.count.lock().add_assign(self.t_count);
            self.skip.lock().add_assign(self.t_skip);
            self.debug_interval.lock().add_assign(1);
        }
        // reset local
        {
            self.t_count = 0;
            self.t_skip = 0;
        }
        // handle debug interval
        {
            if (*self.debug_interval.lock()).is_multiple_of(DEBUG_INTERVAL) {
                trace!(
                    "Processed {} records; skipped {}",
                    self.count.lock(),
                    self.skip.lock()
                );
            }
        }
    }

    pub fn finish(&mut self) -> binseq::Result<()> {
        self.writer.lock().finish()
    }

    pub fn get_global_record_count(&self) -> usize {
        *self.count.lock()
    }

    pub fn get_global_skip_count(&self) -> usize {
        *self.skip.lock()
    }
}

impl<W: Write + Send, Rf: paraseq::Record> ParallelProcessor<Rf> for Encoder<W> {
    fn process_record(&mut self, record: Rf) -> paraseq::Result<()> {
        let seq = record.seq();
        let rec = SequencingRecordBuilder::default()
            .s_seq(&seq)
            .opt_s_qual(record.qual())
            .s_header(record.id())
            .build()
            .map_err(IntoProcessError::into_process_error)?;
        if self
            .t_writer
            .push(rec)
            .map_err(IntoProcessError::into_process_error)?
        {
            self.t_count += 1;
        } else {
            self.t_skip += 1;
        }
        Ok(())
    }
    fn on_batch_complete(&mut self) -> paraseq::Result<()> {
        self.update_global_counters();
        self.write_batch()
            .map_err(IntoProcessError::into_process_error)
    }
}

impl<W: Write + Send, Rf: paraseq::Record> PairedParallelProcessor<Rf> for Encoder<W> {
    fn process_record_pair(&mut self, record1: Rf, record2: Rf) -> paraseq::Result<()> {
        let s_seq = record1.seq();
        let x_seq = record2.seq();
        let rec = SequencingRecordBuilder::default()
            .s_seq(&s_seq)
            .opt_s_qual(record1.qual())
            .s_header(record1.id())
            .x_seq(&x_seq)
            .opt_x_qual(record2.qual())
            .x_header(record2.id())
            .build()
            .map_err(IntoProcessError::into_process_error)?;
        if self
            .t_writer
            .push(rec)
            .map_err(IntoProcessError::into_process_error)?
        {
            self.t_count += 1;
        } else {
            self.t_skip += 1;
        }
        Ok(())
    }
    fn on_batch_complete(&mut self) -> paraseq::Result<()> {
        self.update_global_counters();
        self.write_batch()
            .map_err(IntoProcessError::into_process_error)
    }
}
impl<W: Write + Send> binseq::ParallelProcessor for Encoder<W> {
    fn process_record<R: binseq::BinseqRecord>(&mut self, record: R) -> binseq::Result<()> {
        let rec = if self.t_writer.is_paired() {
            SequencingRecordBuilder::default()
                .s_seq(record.sseq())
                .opt_s_qual(record.has_quality().then(|| record.squal()))
                .s_header(record.sheader())
                .x_seq(record.xseq())
                .opt_x_qual(record.has_quality().then(|| record.xqual()))
                .x_header(record.xheader())
                .build()?
        } else {
            SequencingRecordBuilder::default()
                .s_seq(record.sseq())
                .opt_s_qual(record.has_quality().then(|| record.squal()))
                .s_header(record.sheader())
                .build()?
        };
        if self.t_writer.push(rec)? {
            self.t_count += 1;
        } else {
            self.t_skip += 1;
        }
        Ok(())
    }
    fn on_batch_complete(&mut self) -> binseq::Result<()> {
        self.update_global_counters();
        self.write_batch()
    }
}
