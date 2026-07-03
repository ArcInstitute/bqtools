use std::{io::Write, path::Path, sync::Arc};

use anyhow::Result;
use binseq::BinseqRecord;
use parking_lot::Mutex;
use serde::Serialize;

use super::report::table;
use crate::commands::{match_output, qc::modules::QcModule, utils::make_directory};

const BASE_CONTENT_PRIMARY_PATH: &str = "base_content_R1.tsv";
const BASE_CONTENT_EXTENDED_PATH: &str = "base_content_R2.tsv";

const NUM_BASES: usize = 5;
const IDX_A: usize = 0;
const IDX_C: usize = 1;
const IDX_G: usize = 2;
const IDX_T: usize = 3;
const IDX_N: usize = 4;

pub type BaseAbundance = [usize; NUM_BASES];
pub const DEFAULT_BASE_ABUNDANCE: BaseAbundance = [0; NUM_BASES];

/// Buckets a decoded base into its histogram index.
///
/// Anything outside `ACGT` (case-insensitive) - ambiguity codes included -
/// is folded into the `N` bucket.
fn base_index(base: u8) -> usize {
    match base {
        b'A' | b'a' => IDX_A,
        b'C' | b'c' => IDX_C,
        b'G' | b'g' => IDX_G,
        b'T' | b't' => IDX_T,
        _ => IDX_N,
    }
}

#[derive(Serialize)]
pub struct BaseContentRecord {
    pos: usize,
    a: usize,
    c: usize,
    g: usize,
    t: usize,
    n: usize,
    pct_a: f64,
    pct_c: f64,
    pct_g: f64,
    pct_t: f64,
    pct_n: f64,
}

#[derive(Debug, Clone, Default)]
pub struct BaseContentHistogram {
    /// Outer: position
    /// Inner: base abundance (A, C, G, T, N)
    inner: Vec<BaseAbundance>,
}
impl BaseContentHistogram {
    /// Checks if empty
    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
    /// Number of positions tracked
    fn len(&self) -> usize {
        self.inner.len()
    }
    /// Track base identity histogram over positions
    fn push(&mut self, seq: &[u8]) {
        if seq.is_empty() {
            return;
        }
        if self.inner.len() < seq.len() {
            self.inner.resize(seq.len(), DEFAULT_BASE_ABUNDANCE);
        }
        seq.iter()
            .zip(self.inner.iter_mut())
            .for_each(|(&base, pos_vec)| {
                pos_vec[base_index(base)] += 1;
            });
    }
    fn ingest(&mut self, other: &mut Self) {
        if self.len() < other.len() {
            self.inner.resize(other.len(), DEFAULT_BASE_ABUNDANCE);
        }
        self.inner
            .iter_mut()
            .zip(other.inner.iter_mut())
            .for_each(|(self_pos, other_pos)| {
                self_pos
                    .iter_mut()
                    .zip(other_pos.iter_mut())
                    .for_each(|(self_c, other_c)| {
                        *self_c += *other_c;
                        *other_c = 0;
                    });
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
            .enumerate()
            .try_for_each(|(pos, counts)| -> Result<()> {
                let total = counts.iter().sum::<usize>() as f64;
                let pct = |c: usize| {
                    if total > 0.0 {
                        (c as f64 / total) * 100.0
                    } else {
                        0.0
                    }
                };
                ser.serialize(&BaseContentRecord {
                    pos,
                    a: counts[IDX_A],
                    c: counts[IDX_C],
                    g: counts[IDX_G],
                    t: counts[IDX_T],
                    n: counts[IDX_N],
                    pct_a: pct(counts[IDX_A]),
                    pct_c: pct(counts[IDX_C]),
                    pct_g: pct(counts[IDX_G]),
                    pct_t: pct(counts[IDX_T]),
                    pct_n: pct(counts[IDX_N]),
                })
                .map_err(Into::into)
            })?;

        ser.flush().map_err(Into::into)
    }

    /// Aggregate base composition across all positions.
    fn totals(&self) -> BaseAbundance {
        let mut totals = DEFAULT_BASE_ABUNDANCE;
        for counts in &self.inner {
            for (t, &c) in totals.iter_mut().zip(counts.iter()) {
                *t += c;
            }
        }
        totals
    }

    fn summary_table(&self) -> Option<String> {
        if self.is_empty() {
            return None;
        }

        let totals = self.totals();
        let total: usize = totals.iter().sum();
        let pct = |c: usize| {
            if total == 0 {
                0.0
            } else {
                (c as f64 / total as f64) * 100.0
            }
        };

        Some(table(
            &["Base", "Count", "Pct"],
            &[
                vec![
                    "A".into(),
                    totals[IDX_A].to_string(),
                    format!("{:.2}%", pct(totals[IDX_A])),
                ],
                vec![
                    "C".into(),
                    totals[IDX_C].to_string(),
                    format!("{:.2}%", pct(totals[IDX_C])),
                ],
                vec![
                    "G".into(),
                    totals[IDX_G].to_string(),
                    format!("{:.2}%", pct(totals[IDX_G])),
                ],
                vec![
                    "T".into(),
                    totals[IDX_T].to_string(),
                    format!("{:.2}%", pct(totals[IDX_T])),
                ],
                vec![
                    "N".into(),
                    totals[IDX_N].to_string(),
                    format!("{:.2}%", pct(totals[IDX_N])),
                ],
            ],
        ))
    }
}

#[derive(Clone, Default)]
pub struct PerBaseSequenceContent {
    /// thread - per base sequence content (primary)
    t_base_content: BaseContentHistogram,
    /// thread - per base sequence content (extended)
    t_base_xcontent: BaseContentHistogram,

    /// global - per base sequence content (primary)
    base_content: Arc<Mutex<BaseContentHistogram>>,
    /// global - per base sequence content (extended)
    base_xcontent: Arc<Mutex<BaseContentHistogram>>,
}
impl QcModule for PerBaseSequenceContent {
    fn push<R: BinseqRecord>(&mut self, record: &R) {
        self.t_base_content.push(record.sseq());
        self.t_base_xcontent.push(record.xseq());
    }

    fn sync_final(&mut self) {
        self.base_content.lock().ingest(&mut self.t_base_content);
        self.base_xcontent.lock().ingest(&mut self.t_base_xcontent);
    }

    fn finish<P: AsRef<Path>>(&mut self, outdir: P) -> Result<()> {
        if !outdir.as_ref().exists() {
            make_directory(outdir.as_ref())?;
        }

        let write_to = |base_content: &BaseContentHistogram, primary: bool| -> Result<()> {
            if base_content.is_empty() {
                return Ok(());
            }
            let mut handle = if primary {
                match_output(Some(outdir.as_ref().join(BASE_CONTENT_PRIMARY_PATH)))
            } else {
                match_output(Some(outdir.as_ref().join(BASE_CONTENT_EXTENDED_PATH)))
            }?;
            base_content.serialize_to(&mut handle)
        };

        write_to(&self.base_content.lock(), true)?;
        write_to(&self.base_xcontent.lock(), false)?;

        Ok(())
    }

    fn summarize(&self) -> String {
        let primary = self.base_content.lock().summary_table();
        let extended = self.base_xcontent.lock().summary_table();
        super::report::dual_section("Per-Base Sequence Content", primary, extended)
    }
}
