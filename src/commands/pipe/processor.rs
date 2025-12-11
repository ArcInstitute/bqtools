use std::{io::Write, sync::Arc, thread};

use anyhow::Result;
use binseq::ParallelProcessor;
use log::trace;
use parking_lot::Mutex;

use super::BoxedWriter;
use crate::{
    cli::FileFormat,
    commands::{decode::write_record, pipe::open_fifo},
};

type SharedWriter = Arc<Mutex<BoxedWriter>>;
type Coord = Arc<Mutex<()>>;

#[derive(Clone)]
pub struct PipeProcessor {
    w1: Option<SharedWriter>,
    w2: Option<SharedWriter>,
    coord: Coord,
    local_w1: Vec<u8>,
    local_w2: Vec<u8>,
    format: FileFormat,
    paired: bool,
    pid: usize,
    tid: usize,
}
impl PipeProcessor {
    pub fn new(basename: &str, pid: usize, format: FileFormat, paired: bool) -> Result<Self> {
        let (w1, w2) = if paired {
            let path_r1 = format!("{}_{}_R1.fq", basename, pid);
            let path_r2 = format!("{}_{}_R2.fq", basename, pid);

            let w1_open_thread =
                thread::spawn(move || -> Result<BoxedWriter> { open_fifo(&path_r1) });
            let w2_open_thread =
                thread::spawn(move || -> Result<BoxedWriter> { open_fifo(&path_r2) });

            let w1 = w1_open_thread.join().unwrap()?;
            let w2 = w2_open_thread.join().unwrap()?;

            let w1 = Arc::new(Mutex::new(w1));
            let w2 = Arc::new(Mutex::new(w2));

            (Some(w1), Some(w2))
        } else {
            let path = format!("{}_{}.fq", basename, pid);
            let w1 = Arc::new(Mutex::new(open_fifo(&path)?));
            let w2 = None;
            (Some(w1), w2)
        };

        Ok(Self {
            w1,
            w2,
            local_w1: Vec::new(),
            local_w2: Vec::new(),
            format,
            paired,
            pid,
            tid: 0,
            coord: Arc::new(Mutex::new(())),
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
        if self.paired {
            write_record(
                &mut self.local_w1,
                record.sheader(),
                record.sseq(),
                record.squal(),
                self.format,
            )?;

            write_record(
                &mut self.local_w2,
                record.xheader(),
                record.xseq(),
                record.xqual(),
                self.format,
            )?;
        } else {
            write_record(
                &mut self.local_w1,
                record.sheader(),
                record.sseq(),
                record.squal(),
                self.format,
            )?;
        }
        Ok(())
    }
    fn on_batch_complete(&mut self) -> binseq::Result<()> {
        if self.paired {
            let coord = self.coord.lock();

            let w1 = Arc::clone(&self.w1.as_ref().unwrap());
            let w2 = Arc::clone(&self.w2.as_ref().unwrap());

            let mut r1_data = Vec::with_capacity(self.local_w1.capacity());
            let mut r2_data = Vec::with_capacity(self.local_w2.capacity());

            std::mem::swap(&mut r1_data, &mut self.local_w1);
            std::mem::swap(&mut r2_data, &mut self.local_w2);

            // Write R1 and R2 in parallel so neither blocks the other
            // let h1_pid = self.pid;
            // let h1_tid = self.tid;
            let h1 = thread::spawn(move || -> binseq::Result<()> {
                let mut lock = w1.lock();
                // trace!("Writing R1 data :: Pipe {} Thread {}...", h1_pid, h1_tid);
                lock.write_all(&r1_data)?;
                // trace!("Flushing R1 data :: Pipe {} Thread {}...", h1_pid, h1_tid);
                lock.flush()?;
                // trace!("R1 data flushed :: Pipe {} Thread {}...", h1_pid, h1_tid);
                Ok(())
            });

            // let h2_pid = self.pid;
            // let h2_tid = self.tid;
            let h2 = thread::spawn(move || -> binseq::Result<()> {
                let mut lock = w2.lock();
                // trace!("Writing R2 data :: Pipe {} Thread {}...", h2_pid, h2_tid);
                lock.write_all(&r2_data)?;
                // trace!("Flushing R2 data :: Pipe {} Thread {}...", h2_pid, h2_tid);
                lock.flush()?;
                // trace!("R2 data flushed :: Pipe {} Thread {}...", h2_pid, h2_tid);
                Ok(())
            });

            // Drop the Coord lock to allow the other thread to proceed
            drop(coord);

            trace!("Joining R1 writer :: Pipe {} Thread {}", self.pid, self.tid);
            h1.join().unwrap()?;
            trace!("Joined R1 writer :: Pipe {} Thread {}", self.pid, self.tid);

            trace!("Joining R2 writer :: Pipe {} Thread {}", self.pid, self.tid);
            h2.join().unwrap()?;
            trace!("Joined R2 writer :: Pipe {} Thread {}", self.pid, self.tid);
        } else {
            let mut lock_w1 = self.w1.as_ref().unwrap().lock();
            lock_w1.write_all(&self.local_w1)?;
            lock_w1.flush()?;
        }
        self.local_w1.clear();
        self.local_w2.clear();
        Ok(())
    }
}
