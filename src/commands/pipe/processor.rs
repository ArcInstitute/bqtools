use std::{io::Write, sync::Arc};

use anyhow::Result;
use binseq::ParallelProcessor;
use parking_lot::Mutex;
// use log::{debug, trace};

use super::BoxedWriter;
use crate::{
    cli::FileFormat,
    commands::{decode::write_record, pipe::open_fifo},
};

type SharedWriter = Arc<Mutex<BoxedWriter>>;

#[derive(Clone)]
pub struct PipeProcessor {
    writer: SharedWriter,
    local: Vec<u8>,
    format: FileFormat,
    primary: bool,
    _pid: usize,
    tid: usize,
}
impl PipeProcessor {
    pub fn new(basename: &str, pid: usize, format: FileFormat, primary: bool) -> Result<Self> {
        let path = format!(
            "{}_{}_R{}.fq",
            basename,
            pid,
            if primary { "1" } else { "2" }
        );
        let writer = Arc::new(Mutex::new(open_fifo(&path)?));
        Ok(Self {
            writer,
            local: Vec::new(),
            format,
            primary,
            _pid: pid,
            tid: 0,
        })
    }
}
impl ParallelProcessor for PipeProcessor {
    fn set_tid(&mut self, tid: usize) {
        self.tid = tid;
    }
    fn get_tid(&self) -> Option<usize> {
        Some(self.tid)
    }
    fn process_record<R: binseq::BinseqRecord>(&mut self, record: R) -> binseq::Result<()> {
        if self.primary {
            write_record(
                &mut self.local,
                record.sheader(),
                record.sseq(),
                record.squal(),
                self.format,
            )?;
        } else {
            write_record(
                &mut self.local,
                record.xheader(),
                record.xseq(),
                record.xqual(),
                self.format,
            )?;
        }
        Ok(())
    }
    fn on_batch_complete(&mut self) -> binseq::Result<()> {
        {
            let mut lock = self.writer.lock();
            lock.write_all(&self.local)?;
            lock.flush()?;
        }
        self.local.clear();
        Ok(())
    }
}
