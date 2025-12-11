use std::{io::Write, sync::Arc};

use binseq::ParallelProcessor;
use parking_lot::Mutex;

use super::BoxedWriter;
use crate::cli::FileFormat;

#[derive(Clone)]
pub struct PipeProcessor {
    writers: Arc<Vec<Mutex<BoxedWriter>>>,
    format: FileFormat,
    paired: bool,
    tid: usize,
}
impl PipeProcessor {
    pub fn new(writers: Vec<BoxedWriter>, format: FileFormat, paired: bool) -> Self {
        let writers = Arc::new(
            writers
                .into_iter()
                .map(|writer| Mutex::new(writer))
                .collect(),
        );
        Self {
            writers,
            format,
            paired,
            tid: 0,
        }
    }
}
impl ParallelProcessor for PipeProcessor {
    fn set_tid(&mut self, tid: usize) {
        self.tid = tid
    }

    fn get_tid(&self) -> Option<usize> {
        Some(self.tid)
    }

    fn process_record<R: binseq::BinseqRecord>(&mut self, record: R) -> binseq::Result<()> {
        let tid = self.get_tid().expect("Error - unable to access thread ID");
        if self.paired {
            let mut writer_r1 = self
                .writers
                .get(tid)
                .expect("Unable to access R1 writer for thread")
                .lock();

            format_write(
                &mut writer_r1,
                record.sheader(),
                record.sseq(),
                record.squal(),
                self.format,
            )?;

            let mut writer_r2 = self
                .writers
                .get(tid + 1)
                .expect("Unable to access R2 writer for thread")
                .lock();

            format_write(
                &mut writer_r2,
                record.xheader(),
                record.xseq(),
                record.xqual(),
                self.format,
            )?;
        } else {
            let mut writer = self
                .writers
                .get(tid)
                .expect("Unable to access writer for thread")
                .lock();

            format_write(
                &mut writer,
                record.sheader(),
                record.sseq(),
                record.squal(),
                self.format,
            )?;
        }
        Ok(())
    }
    fn on_batch_complete(&mut self) -> binseq::Result<()> {
        let tid = self.get_tid().expect("Unable to access thread ID");
        if self.paired {
            self.writers
                .get(tid)
                .expect("Unable to access R1 writer for thread")
                .lock()
                .flush()?;
            self.writers
                .get(tid + 1)
                .expect("Unable to access R2 writer for thread")
                .lock()
                .flush()?;
        } else {
            self.writers
                .get(tid)
                .expect("Unable to access writer for thread")
                .lock()
                .flush()?;
        }
        Ok(())
    }
}

fn write_fastq_parts(
    writer: &mut BoxedWriter,
    index: &[u8],
    sequence: &[u8],
    quality: &[u8],
) -> std::io::Result<()> {
    writer.write_all(b"@")?;
    writer.write_all(index)?;
    writer.write_all(b"\n")?;
    writer.write_all(sequence)?;
    writer.write_all(b"\n+\n")?;
    writer.write_all(quality)?;
    writer.write_all(b"\n")?;
    Ok(())
}

fn write_fasta_parts(
    writer: &mut BoxedWriter,
    index: &[u8],
    sequence: &[u8],
) -> std::io::Result<()> {
    writer.write_all(b">")?;
    writer.write_all(index)?;
    writer.write_all(b"\n")?;
    writer.write_all(sequence)?;
    writer.write_all(b"\n")?;
    Ok(())
}

fn format_write(
    writer: &mut BoxedWriter,
    index: &[u8],
    sequence: &[u8],
    quality: &[u8],
    format: FileFormat,
) -> std::io::Result<()> {
    match format {
        FileFormat::Fastq => write_fastq_parts(writer, index, sequence, quality),
        FileFormat::Fasta => write_fasta_parts(writer, index, sequence),
        _ => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Unsupported format",
        )),
    }
}
