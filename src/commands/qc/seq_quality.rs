use anyhow::Result;
use binseq::BinseqRecord;
use parking_lot::Mutex;
use serde::Serialize;
use std::{io::Write, ops::Div, path::Path, sync::Arc};

use super::{QualAbundance, DEFAULT_QUAL_ABUNDANCE, PHRED_OFFSET};
use crate::commands::{match_output, qc::modules::QcModule, utils::make_directory};

const SQ_PRIMARY_PATH: &'static str = "sq_R1.tsv";
const SQ_EXTENDED_PATH: &'static str = "sq_R2.tsv";

#[derive(Clone)]
pub struct QualHistogram {
    inner: QualAbundance,
}
impl Default for QualHistogram {
    fn default() -> Self {
        Self {
            inner: DEFAULT_QUAL_ABUNDANCE,
        }
    }
}
impl QualHistogram {
    fn is_empty(&self) -> bool {
        self.inner.iter().copied().sum::<usize>() == 0
    }

    fn push(&mut self, qual: &[u8]) {
        if qual.is_empty() {
            return;
        }
        let total: usize = qual
            .iter()
            .map(|x| x.saturating_sub(PHRED_OFFSET) as usize)
            .sum();
        let binned_mean = (total as f64).div(&(qual.len() as f64)).round() as usize;
        self.inner[binned_mean.min(self.inner.len() - 1)] += 1;
    }

    fn ingest(&mut self, other: &mut Self) {
        self.inner
            .iter_mut()
            .zip(other.inner.iter_mut())
            .for_each(|(u, v)| {
                *u += *v;
                *v = 0;
            })
    }

    fn serialize_to<W: Write>(&self, wtr: &mut W) -> Result<()> {
        if self.is_empty() {
            return Ok(());
        }

        let mut ser = csv::WriterBuilder::default()
            .delimiter(b'\t')
            .has_headers(true)
            .from_writer(wtr);

        #[derive(Serialize)]
        struct Record {
            qual: usize,
            count: usize,
        }

        self.inner
            .iter()
            .copied()
            .enumerate()
            .try_for_each(|(qual, count)| -> Result<()> {
                ser.serialize(&Record { qual, count }).map_err(Into::into)
            })?;

        ser.flush().map_err(Into::into)
    }
}

#[derive(Default, Clone)]
pub struct PerSequenceQuality {
    t_seq_squal: QualHistogram,
    t_seq_xqual: QualHistogram,

    seq_squal: Arc<Mutex<QualHistogram>>,
    seq_xqual: Arc<Mutex<QualHistogram>>,
}
impl QcModule for PerSequenceQuality {
    fn push<R: BinseqRecord>(&mut self, record: &R) {
        self.t_seq_squal.push(record.squal());
        self.t_seq_xqual.push(record.xqual());
    }

    fn sync(&mut self) {
        self.seq_squal.lock().ingest(&mut self.t_seq_squal);
        self.seq_xqual.lock().ingest(&mut self.t_seq_xqual);
    }

    fn finish<P: AsRef<Path>>(&mut self, outdir: P) -> Result<()> {
        if !outdir.as_ref().exists() {
            make_directory(outdir.as_ref())?;
        }

        let write_to = |seq_qual: &QualHistogram, primary: bool| -> Result<()> {
            if seq_qual.is_empty() {
                return Ok(());
            }
            let mut handle = if primary {
                match_output(Some(outdir.as_ref().join(SQ_PRIMARY_PATH)))
            } else {
                match_output(Some(outdir.as_ref().join(SQ_EXTENDED_PATH)))
            }?;
            seq_qual.serialize_to(&mut handle)
        };

        write_to(&self.seq_squal.lock(), true)?;
        write_to(&self.seq_xqual.lock(), false)?;

        Ok(())
    }
}
