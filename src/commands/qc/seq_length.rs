use std::{io::Write, path::Path, sync::Arc};

use anyhow::Result;
use binseq::BinseqRecord;
use parking_lot::Mutex;
use serde::Serialize;

use crate::commands::{match_output, qc::modules::QcModule, utils::make_directory};

const SLEN_PRIMARY_PATH: &str = "slen_R1.tsv";
const SLEN_EXTENDED_PATH: &str = "slen_R2.tsv";

#[derive(Serialize)]
pub struct SeqLenRecord {
    len: usize,
    count: usize,
}

#[derive(Debug, Clone, Default)]
pub struct SeqLenHistogram {
    /// Indexed directly by sequence length
    inner: Vec<usize>,
}
impl SeqLenHistogram {
    fn is_empty(&self) -> bool {
        self.inner.iter().copied().sum::<usize>() == 0
    }
    fn len(&self) -> usize {
        self.inner.len()
    }
    /// Track a single read's length
    fn push(&mut self, len: usize) {
        if len == 0 {
            return;
        }
        if self.inner.len() <= len {
            self.inner.resize(len + 1, 0);
        }
        self.inner[len] += 1;
    }
    fn ingest(&mut self, other: &mut Self) {
        if self.len() < other.len() {
            self.inner.resize(other.len(), 0);
        }
        self.inner
            .iter_mut()
            .zip(other.inner.iter_mut())
            .for_each(|(u, v)| {
                *u += *v;
                *v = 0;
            });
    }
    fn serialize_to<W: Write>(&self, wtr: &mut W) -> Result<()> {
        if self.is_empty() {
            return Ok(());
        }

        let mut ser = csv::WriterBuilder::default()
            .delimiter(b'\t')
            .has_headers(true)
            .from_writer(wtr);

        self.inner
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, count)| *count > 0)
            .try_for_each(|(len, count)| -> Result<()> {
                ser.serialize(&SeqLenRecord { len, count })
                    .map_err(Into::into)
            })?;

        ser.flush().map_err(Into::into)
    }
}

#[derive(Clone, Default)]
pub struct SequenceLengthDistribution {
    /// thread - sequence length distribution (primary)
    t_slen: SeqLenHistogram,
    /// thread - sequence length distribution (extended)
    t_xlen: SeqLenHistogram,

    /// global - sequence length distribution (primary)
    slen: Arc<Mutex<SeqLenHistogram>>,
    /// global - sequence length distribution (extended)
    xlen: Arc<Mutex<SeqLenHistogram>>,
}
impl QcModule for SequenceLengthDistribution {
    fn push<R: BinseqRecord>(&mut self, record: &R) {
        self.t_slen.push(record.slen() as usize);
        self.t_xlen.push(record.xlen() as usize);
    }

    fn sync(&mut self) {
        self.slen.lock().ingest(&mut self.t_slen);
        self.xlen.lock().ingest(&mut self.t_xlen);
    }

    fn finish<P: AsRef<Path>>(&mut self, outdir: P) -> Result<()> {
        if !outdir.as_ref().exists() {
            make_directory(outdir.as_ref())?;
        }

        let write_to = |hist: &SeqLenHistogram, primary: bool| -> Result<()> {
            if hist.is_empty() {
                return Ok(());
            }
            let mut handle = if primary {
                match_output(Some(outdir.as_ref().join(SLEN_PRIMARY_PATH)))
            } else {
                match_output(Some(outdir.as_ref().join(SLEN_EXTENDED_PATH)))
            }?;
            hist.serialize_to(&mut handle)
        };

        write_to(&self.slen.lock(), true)?;
        write_to(&self.xlen.lock(), false)?;

        Ok(())
    }
}
