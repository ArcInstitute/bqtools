use std::{io::Write, sync::Arc};

use binseq::{BinseqRecord, BinseqWriter, ParallelProcessor, SequencingRecordBuilder};
use parking_lot::Mutex;

use crate::cli::Mate;

/// Reverse complements a nucleotide sequence buffer in place.
///
/// Any byte outside `ACGTacgt` (e.g. `N`) is left untouched, matching the
/// behavior of 4-bit decoding, which collapses all ambiguity codes to `N`.
fn reverse_complement(buf: &mut [u8]) {
    buf.reverse();
    for base in buf.iter_mut() {
        *base = match *base {
            b'A' => b'T',
            b'C' => b'G',
            b'G' => b'C',
            b'T' => b'A',
            b'a' => b't',
            b'c' => b'g',
            b'g' => b'c',
            b't' => b'a',
            other => other,
        };
    }
}

pub struct RevCompProcessor<W: Write + Send> {
    /// Which mate(s) to reverse complement
    mate: Mate,

    /// Thread-local writer for the processor
    t_writer: BinseqWriter<Vec<u8>>,
    /// Thread-local record count
    t_count: usize,

    /// Thread-local scratch buffers for the transformed primary sequence/quality
    sseq: Vec<u8>,
    squal: Vec<u8>,
    /// Thread-local scratch buffers for the transformed extended sequence/quality
    xseq: Vec<u8>,
    xqual: Vec<u8>,

    /// Global writer for the processor
    writer: Arc<Mutex<BinseqWriter<W>>>,
    /// Global record count
    count: Arc<Mutex<usize>>,
}
impl<W: Write + Send> Clone for RevCompProcessor<W> {
    fn clone(&self) -> Self {
        Self {
            mate: self.mate,
            t_writer: self.t_writer.clone(),
            t_count: 0,
            sseq: Vec::new(),
            squal: Vec::new(),
            xseq: Vec::new(),
            xqual: Vec::new(),
            writer: self.writer.clone(),
            count: self.count.clone(),
        }
    }
}
impl<W: Write + Send> RevCompProcessor<W> {
    pub fn new(writer: BinseqWriter<W>, mate: Mate) -> binseq::Result<Self> {
        let t_writer = writer.new_headless_buffer()?;
        Ok(Self {
            mate,
            t_writer,
            t_count: 0,
            sseq: Vec::new(),
            squal: Vec::new(),
            xseq: Vec::new(),
            xqual: Vec::new(),
            writer: Arc::new(Mutex::new(writer)),
            count: Arc::new(Mutex::new(0)),
        })
    }

    fn write_batch(&mut self) -> binseq::Result<()> {
        self.writer.lock().ingest_completed(&mut self.t_writer)
    }

    fn write_final(&mut self) -> binseq::Result<()> {
        self.writer.lock().ingest(&mut self.t_writer)
    }

    pub fn finish(&mut self) -> binseq::Result<()> {
        self.writer.lock().finish()
    }

    pub fn get_global_record_count(&self) -> usize {
        *self.count.lock()
    }
}

impl<W: Write + Send> ParallelProcessor for RevCompProcessor<W> {
    fn process_record<B: BinseqRecord>(&mut self, record: B) -> binseq::Result<()> {
        let is_paired = record.is_paired();
        let has_quality = record.has_quality();
        let rc_primary = matches!(self.mate, Mate::One | Mate::Both);
        let rc_extended = is_paired && matches!(self.mate, Mate::Two | Mate::Both);

        if rc_primary {
            self.sseq.clear();
            self.sseq.extend_from_slice(record.sseq());
            reverse_complement(&mut self.sseq);
            if has_quality {
                self.squal.clear();
                self.squal.extend_from_slice(record.squal());
                self.squal.reverse();
            }
        }
        if rc_extended {
            self.xseq.clear();
            self.xseq.extend_from_slice(record.xseq());
            reverse_complement(&mut self.xseq);
            if has_quality {
                self.xqual.clear();
                self.xqual.extend_from_slice(record.xqual());
                self.xqual.reverse();
            }
        }

        let s_seq: &[u8] = if rc_primary {
            &self.sseq
        } else {
            record.sseq()
        };
        let s_qual: Option<&[u8]> = if !has_quality {
            None
        } else if rc_primary {
            Some(&self.squal)
        } else {
            Some(record.squal())
        };

        let rec = if is_paired {
            let x_seq: &[u8] = if rc_extended {
                &self.xseq
            } else {
                record.xseq()
            };
            let x_qual: Option<&[u8]> = if !has_quality {
                None
            } else if rc_extended {
                Some(&self.xqual)
            } else {
                Some(record.xqual())
            };
            SequencingRecordBuilder::default()
                .s_seq(s_seq)
                .opt_s_qual(s_qual)
                .s_header(record.sheader())
                .x_seq(x_seq)
                .opt_x_qual(x_qual)
                .x_header(record.xheader())
                .build()?
        } else {
            SequencingRecordBuilder::default()
                .s_seq(s_seq)
                .opt_s_qual(s_qual)
                .s_header(record.sheader())
                .build()?
        };

        if self.t_writer.push(rec)? {
            self.t_count += 1;
        }
        Ok(())
    }

    fn on_batch_complete(&mut self) -> binseq::Result<()> {
        *self.count.lock() += self.t_count;
        self.t_count = 0;
        self.write_batch()
    }

    fn on_thread_complete(&mut self) -> binseq::Result<()> {
        self.write_final()
    }
}

#[cfg(test)]
mod tests {
    use super::reverse_complement;

    #[test]
    fn test_reverse_complement_basic() {
        let mut seq = b"ACGTACGT".to_vec();
        reverse_complement(&mut seq);
        assert_eq!(seq, b"ACGTACGT");

        let mut seq = b"GATTACA".to_vec();
        reverse_complement(&mut seq);
        assert_eq!(seq, b"TGTAATC");
    }

    #[test]
    fn test_reverse_complement_preserves_n() {
        let mut seq = b"ACGTN".to_vec();
        reverse_complement(&mut seq);
        assert_eq!(seq, b"NACGT");
    }
}
