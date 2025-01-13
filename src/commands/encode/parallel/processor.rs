use std::sync::{atomic::AtomicUsize, Arc, Mutex};

use anyhow::{bail, Result};
use binseq::{
    writer::{write_buffer, write_flag},
    BinseqHeader,
};
use seq_io_parallel::{MinimalRefRecord, PairedParallelProcessor, ParallelProcessor};

use crate::commands::reopen_output;

#[derive(Debug, Clone)]
pub struct Processor {
    /// Header of the global record set
    header: BinseqHeader,

    /// Optional output path
    path: Option<String>,

    /// Local buffer for encoding (used by individual threads)
    ebuf_r1: Vec<u64>,

    /// Local buffer for encoding (used by individual threads)
    ebuf_r2: Vec<u64>,

    /// Local buffer for write queue
    wbuf: Vec<u8>,

    /// Lock for writing to the output
    writing: Arc<Mutex<()>>,

    /// local variables for number of records processed
    local_num_records: usize,
    local_num_skipped: usize,

    /// global variables for number of records processed
    global_num_records: Arc<AtomicUsize>,
    global_num_skipped: Arc<AtomicUsize>,
}
impl Processor {
    pub fn new(header: BinseqHeader, path: Option<String>) -> Self {
        Self {
            header,
            path,
            ebuf_r1: Vec::new(),
            ebuf_r2: Vec::new(),
            wbuf: Vec::new(),
            writing: Arc::new(Mutex::new(())),
            local_num_records: 0,
            local_num_skipped: 0,
            global_num_records: Arc::new(AtomicUsize::new(0)),
            global_num_skipped: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn write_batch(&mut self) -> Result<()> {
        // Write the buffer to the output
        {
            let _lock = self.writing.lock().unwrap();
            let mut out_handle = reopen_output(self.path.as_ref()).unwrap();
            out_handle.write_all(&self.wbuf)?;
            out_handle.flush()?;
        }

        // Clear the buffer
        self.wbuf.clear();

        Ok(())
    }

    fn update_global_counts(&mut self) {
        self.global_num_records
            .fetch_add(self.local_num_records, std::sync::atomic::Ordering::Relaxed);
        self.global_num_skipped
            .fetch_add(self.local_num_skipped, std::sync::atomic::Ordering::Relaxed);

        self.local_num_records = 0;
        self.local_num_skipped = 0;
    }

    pub fn get_global_num_records(&self) -> usize {
        self.global_num_records
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn get_global_num_skipped(&self) -> usize {
        self.global_num_skipped
            .load(std::sync::atomic::Ordering::Relaxed)
    }
}
impl ParallelProcessor for Processor {
    fn process_record<'a, Rf: MinimalRefRecord<'a>>(&mut self, record: Rf) -> Result<()> {
        self.ebuf_r1.clear();

        if record.ref_seq().len() != self.header.slen as usize {
            panic!("Record length mismatch");
        }

        if bitnuc::encode(record.ref_seq(), &mut self.ebuf_r1).is_ok() {
            // Write the encoded sequence to the output
            write_flag(&mut self.wbuf, 0)?;
            write_buffer(&mut self.wbuf, &self.ebuf_r1)?;
            self.local_num_records += 1;
        } else {
            self.local_num_skipped += 1;
        }

        // implicitly skip the record if encoding fails
        Ok(())
    }

    fn on_batch_complete(&mut self) -> Result<()> {
        self.update_global_counts();
        self.write_batch()
    }
}
impl PairedParallelProcessor for Processor {
    fn process_record_pair<'a, Rf: MinimalRefRecord<'a>>(&mut self, r1: Rf, r2: Rf) -> Result<()> {
        self.ebuf_r1.clear();
        self.ebuf_r2.clear();

        if r1.ref_seq().len() != self.header.slen as usize {
            bail!(
                "Record length mismatch (R1): expected ({}), observed ({})",
                self.header.slen,
                r1.ref_seq().len(),
            )
        }
        if r2.ref_seq().len() != self.header.xlen as usize {
            bail!(
                "Record length mismatch (R2): expected ({}), observed ({})",
                self.header.xlen,
                r2.ref_seq().len()
            )
        }

        if bitnuc::encode(r1.ref_seq(), &mut self.ebuf_r1).is_ok()
            && bitnuc::encode(r2.ref_seq(), &mut self.ebuf_r2).is_ok()
        {
            // Write the encoded sequence to the output
            write_flag(&mut self.wbuf, 0)?;
            write_buffer(&mut self.wbuf, &self.ebuf_r1)?;
            write_buffer(&mut self.wbuf, &self.ebuf_r2)?;
            self.local_num_records += 1;
        } else {
            self.local_num_skipped += 1;
        }

        // implicitly skip the record if encoding fails
        Ok(())
    }

    fn on_batch_complete(&mut self) -> Result<()> {
        self.update_global_counts();
        self.write_batch()
    }
}
