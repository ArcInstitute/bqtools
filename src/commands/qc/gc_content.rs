use anyhow::Result;
use binseq::BinseqRecord;
use parking_lot::Mutex;
use serde::Serialize;
use std::{io::Write, path::Path, sync::Arc};

use super::report::table;
use crate::commands::{match_output, qc::modules::QcModule, utils::make_directory};

const GC_CONTENT_PRIMARY_PATH: &str = "gc_content_R1.tsv";
const GC_CONTENT_EXTENDED_PATH: &str = "gc_content_R2.tsv";

/// Percentage bins: 0..=100
const NUM_GC_BINS: usize = 101;

pub type GcAbundance = [usize; NUM_GC_BINS];
pub const DEFAULT_GC_ABUNDANCE: GcAbundance = [0; NUM_GC_BINS];

fn is_gc(base: u8) -> bool {
    matches!(base, b'G' | b'g' | b'C' | b'c')
}

#[derive(Serialize)]
struct GcContentRecord {
    pct_gc: usize,
    count: usize,
}

#[derive(Clone)]
pub struct GcHistogram {
    inner: GcAbundance,
}
impl Default for GcHistogram {
    fn default() -> Self {
        Self {
            inner: DEFAULT_GC_ABUNDANCE,
        }
    }
}
impl GcHistogram {
    fn is_empty(&self) -> bool {
        self.inner.iter().copied().sum::<usize>() == 0
    }

    /// Bin a whole read by the percentage of G/C bases it contains.
    fn push(&mut self, seq: &[u8]) {
        if seq.is_empty() {
            return;
        }
        let gc = seq.iter().copied().filter(|&b| is_gc(b)).count();
        let pct_gc = ((gc as f64 / seq.len() as f64) * 100.0).round() as usize;
        self.inner[pct_gc.min(self.inner.len() - 1)] += 1;
    }

    fn ingest(&mut self, other: &mut Self) {
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
            .try_for_each(|(pct_gc, count)| -> Result<()> {
                ser.serialize(&GcContentRecord { pct_gc, count })
                    .map_err(Into::into)
            })?;

        ser.flush().map_err(Into::into)
    }

    fn total(&self) -> usize {
        self.inner.iter().sum()
    }

    fn mean(&self) -> f64 {
        let total = self.total();
        if total == 0 {
            0.0
        } else {
            let sum: usize = self.inner.iter().enumerate().map(|(pct, &c)| pct * c).sum();
            sum as f64 / total as f64
        }
    }

    fn median(&self) -> usize {
        let total = self.total();
        if total == 0 {
            return 0;
        }
        let half = total / 2;
        let mut cum = 0;
        for (pct, &c) in self.inner.iter().enumerate() {
            cum += c;
            if cum > half {
                return pct;
            }
        }
        0
    }

    fn mode(&self) -> usize {
        self.inner
            .iter()
            .enumerate()
            .max_by_key(|&(_, &c)| c)
            .map_or(0, |(pct, _)| pct)
    }

    fn summary_table(&self) -> Option<String> {
        if self.is_empty() {
            return None;
        }
        Some(table(
            &["Metric", "Value"],
            &[
                vec!["Reads".into(), self.total().to_string()],
                vec!["Mean GC%".into(), format!("{:.2}%", self.mean())],
                vec!["Median GC%".into(), format!("{}%", self.median())],
                vec!["Mode GC%".into(), format!("{}%", self.mode())],
            ],
        ))
    }
}

#[derive(Default, Clone)]
pub struct PerSequenceGcContent {
    t_seq_gc: GcHistogram,
    t_seq_xgc: GcHistogram,

    seq_gc: Arc<Mutex<GcHistogram>>,
    seq_xgc: Arc<Mutex<GcHistogram>>,
}
impl QcModule for PerSequenceGcContent {
    fn push<R: BinseqRecord>(&mut self, record: &R) {
        self.t_seq_gc.push(record.sseq());
        self.t_seq_xgc.push(record.xseq());
    }

    fn sync_final(&mut self) {
        self.seq_gc.lock().ingest(&mut self.t_seq_gc);
        self.seq_xgc.lock().ingest(&mut self.t_seq_xgc);
    }

    fn finish<P: AsRef<Path>>(&mut self, outdir: P) -> Result<()> {
        if !outdir.as_ref().exists() {
            make_directory(outdir.as_ref())?;
        }

        let write_to = |seq_gc: &GcHistogram, primary: bool| -> Result<()> {
            if seq_gc.is_empty() {
                return Ok(());
            }
            let mut handle = if primary {
                match_output(Some(outdir.as_ref().join(GC_CONTENT_PRIMARY_PATH)))
            } else {
                match_output(Some(outdir.as_ref().join(GC_CONTENT_EXTENDED_PATH)))
            }?;
            seq_gc.serialize_to(&mut handle)
        };

        write_to(&self.seq_gc.lock(), true)?;
        write_to(&self.seq_xgc.lock(), false)?;

        Ok(())
    }

    fn summarize(&self) -> String {
        let primary = self.seq_gc.lock().summary_table();
        let extended = self.seq_xgc.lock().summary_table();
        super::report::dual_section("Per-Sequence GC Content", primary, extended)
    }
}
