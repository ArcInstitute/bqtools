use std::{io::Write, sync::Arc};

use anyhow::Result;
use binseq::ParallelProcessor;
use parking_lot::Mutex;

use super::{BoxedWriter, RecordPair};
use crate::{
    cli::FileFormat,
    commands::{
        decode::write_record,
        pipe::utils::{name_fifo, open_fifo},
    },
};

type SharedWriter = Arc<Mutex<BoxedWriter>>;

#[derive(Clone)]
pub struct PipeProcessor {
    writer: SharedWriter,
    local: Vec<u8>,
    format: FileFormat,
    pair: RecordPair,
}
impl PipeProcessor {
    pub fn new(basename: &str, pid: usize, format: FileFormat, pair: RecordPair) -> Result<Self> {
        let path = name_fifo(basename, pid, pair, format);
        let writer = Arc::new(Mutex::new(open_fifo(&path)?));
        Ok(Self {
            writer,
            local: Vec::new(),
            format,
            pair,
        })
    }
}
impl ParallelProcessor for PipeProcessor {
    fn process_record<R: binseq::BinseqRecord>(&mut self, record: R) -> binseq::Result<()> {
        match self.pair {
            RecordPair::Unpaired | RecordPair::R1 => {
                write_record(
                    &mut self.local,
                    record.sheader(),
                    record.sseq(),
                    record.squal(),
                    self.format,
                )?;
            }
            RecordPair::R2 => {
                write_record(
                    &mut self.local,
                    record.xheader(),
                    record.xseq(),
                    record.xqual(),
                    self.format,
                )?;
            }
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
