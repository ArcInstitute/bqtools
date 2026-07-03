use std::{io::Write, path::Path, sync::Arc};

use anyhow::Result;
use binseq::BinseqRecord;
use hashbrown::HashMap;
use log::trace;
use parking_lot::Mutex;
use serde::Serialize;

use super::report::{dual_section, table};
use crate::commands::{match_output, qc::modules::QcModule, utils::make_directory};

const DUPLICATION_LEVELS_PRIMARY_PATH: &str = "duplication_levels_R1.tsv";
const DUPLICATION_LEVELS_EXTENDED_PATH: &str = "duplication_levels_R2.tsv";
const OVERREPRESENTED_PRIMARY_PATH: &str = "overrepresented_sequences_R1.tsv";
const OVERREPRESENTED_EXTENDED_PATH: &str = "overrepresented_sequences_R2.tsv";

/// Number of leading records (by global file index) considered for
/// duplication and overrepresented-sequence analysis. Bounding this keeps
/// memory flat regardless of file size - mirrors `FastQC`'s own subsampling
/// behavior for these modules.
pub const DEFAULT_DUP_SAMPLE_SIZE: usize = 100_000;

/// FastQC-style duplication level buckets: exact counts 1-9, then cumulative
/// thresholds beyond that.
const LEVELS: &[usize] = &[
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 50, 100, 500, 1000, 5000, 10000,
];
const LABELS: &[&str] = &[
    "1", "2", "3", "4", "5", "6", "7", "8", "9", ">10", ">50", ">100", ">500", ">1k", ">5k", ">10k",
];

/// A sequence occurring in at least this fraction of sampled reads is
/// reported as overrepresented - mirrors `FastQC`'s own default threshold.
/// User-configurable via `--overrepresented-threshold`.
pub const DEFAULT_OVERREPRESENTED_THRESHOLD_PCT: f64 = 0.1;

fn pct(n: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        (n as f64 / total as f64) * 100.0
    }
}

/// Max number of overrepresented sequences shown in the summary report (the
/// full list still goes to the TSV).
const OVERREPRESENTED_SUMMARY_LIMIT: usize = 5;

/// Truncates a sequence for display in the summary report so a single long
/// read doesn't blow out the table width.
fn truncate_sequence(seq: &[u8]) -> String {
    const MAX_LEN: usize = 40;
    let seq = String::from_utf8_lossy(seq);
    if seq.len() > MAX_LEN {
        format!("{}...", &seq[..MAX_LEN])
    } else {
        seq.into_owned()
    }
}

/// One row of the duplication-level report (see [`LEVELS`]/[`LABELS`]).
///
/// A read that occurs `N` times in the sample contributes **one** unit to
/// the `distinct_*` columns (it's one distinct sequence) but **`N`** units
/// to the `total_*` columns (it's `N` of the sampled reads). So a file
/// dominated by a handful of highly-duplicated sequences shows up as low
/// counts/percentages in `distinct_*` but high counts/percentages in
/// `total_*` for the same bucket.
#[derive(Serialize)]
struct DuplicationRecord {
    /// Duplication level bucket this row summarizes (exact counts `1`-`9`,
    /// then cumulative thresholds: `>10`, `>50`, ..., `>10k`).
    level: &'static str,
    /// Number of distinct (unique) sequences whose occurrence count falls
    /// in this bucket.
    distinct_count: usize,
    /// `distinct_count` as a percentage of all distinct sequences sampled.
    distinct_pct: f64,
    /// Total sampled reads accounted for by sequences in this bucket, i.e.
    /// `sum(occurrence count)` over every distinct sequence in the bucket.
    total_count: usize,
    /// `total_count` as a percentage of all sampled reads.
    total_pct: f64,
}

/// One row of the overrepresented-sequences report: a single exact sequence
/// that met the configured overrepresented threshold, sorted most-frequent
/// first.
#[derive(Serialize)]
struct OverrepresentedRecord {
    sequence: String,
    /// Number of sampled reads with this exact sequence.
    count: usize,
    /// `count` as a percentage of all sampled reads.
    pct: f64,
}

/// Counts exact-sequence occurrences.
///
/// Keyed on the sequence itself (not a hash of it), so counts can never be
/// corrupted by a hash collision - two entries only ever merge because the
/// bytes are actually identical.
#[derive(Clone, Default)]
struct DuplicationCounter {
    inner: HashMap<Box<[u8]>, u32>,
}
impl DuplicationCounter {
    fn push(&mut self, seq: &[u8]) {
        if let Some(count) = self.inner.get_mut(seq) {
            *count += 1;
        } else {
            self.inner.insert(seq.into(), 1);
        }
    }
    /// Merges `other`'s counts into `self`, consuming `other`.
    ///
    /// This runs once per thread at `sync_final` (not per batch), by which
    /// point `other` (a thread's local counts) is done accumulating for
    /// good - a plain drain is correct and there's no reason to keep its
    /// keys around for reuse.
    fn ingest(&mut self, other: &mut Self) {
        for (seq, count) in other.inner.drain() {
            *self.inner.entry(seq).or_insert(0) += count;
        }
    }
    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
    /// Writes the bucketed duplication-level report (see [`DuplicationRecord`]).
    fn serialize_levels_to<W: Write>(&self, wtr: &mut W) -> Result<()> {
        if self.is_empty() {
            return Ok(());
        }

        let mut distinct_buckets = vec![0usize; LEVELS.len()];
        let mut total_buckets = vec![0usize; LEVELS.len()];
        let total_distinct = self.inner.len();
        let total_reads: usize = self.inner.values().map(|&count| count as usize).sum();

        for &count in self.inner.values() {
            let count = count as usize;
            let idx = LEVELS.iter().rposition(|&lvl| lvl <= count).unwrap_or(0);
            distinct_buckets[idx] += 1;
            total_buckets[idx] += count;
        }

        let mut ser = csv::WriterBuilder::default()
            .delimiter(b'\t')
            .has_headers(true)
            .from_writer(wtr);

        LABELS
            .iter()
            .enumerate()
            .try_for_each(|(idx, &level)| -> Result<()> {
                ser.serialize(&DuplicationRecord {
                    level,
                    distinct_count: distinct_buckets[idx],
                    distinct_pct: pct(distinct_buckets[idx], total_distinct),
                    total_count: total_buckets[idx],
                    total_pct: pct(total_buckets[idx], total_reads),
                })
                .map_err(Into::into)
            })?;

        ser.flush().map_err(Into::into)
    }

    /// Sequences at or above `threshold_pct` of the sample, most frequent
    /// first, paired with their percentage.
    fn overrepresented(&self, threshold_pct: f64) -> Vec<(&[u8], usize, f64)> {
        let total_reads: usize = self.inner.values().map(|&count| count as usize).sum();

        let mut overrepresented: Vec<(&[u8], usize, f64)> = self
            .inner
            .iter()
            .map(|(seq, &count)| {
                (
                    seq.as_ref(),
                    count as usize,
                    pct(count as usize, total_reads),
                )
            })
            .filter(|&(_, _, pct)| pct >= threshold_pct)
            .collect();
        overrepresented.sort_unstable_by_key(|&(_, count, _)| std::cmp::Reverse(count));
        overrepresented
    }

    /// Writes the overrepresented-sequences report (see [`Self::overrepresented`]).
    /// This is the same underlying counts as [`Self::serialize_levels_to`],
    /// just filtered and listed individually instead of bucketed.
    fn serialize_overrepresented_to<W: Write>(
        &self,
        wtr: &mut W,
        threshold_pct: f64,
    ) -> Result<()> {
        let mut ser = csv::WriterBuilder::default()
            .delimiter(b'\t')
            .has_headers(true)
            .from_writer(wtr);

        self.overrepresented(threshold_pct)
            .into_iter()
            .try_for_each(|(seq, count, pct)| -> Result<()> {
                ser.serialize(&OverrepresentedRecord {
                    sequence: String::from_utf8_lossy(seq).into_owned(),
                    count,
                    pct,
                })
                .map_err(Into::into)
            })?;

        ser.flush().map_err(Into::into)
    }

    fn total_reads(&self) -> usize {
        self.inner.values().map(|&count| count as usize).sum()
    }

    fn summary_table(&self) -> Option<String> {
        if self.is_empty() {
            return None;
        }
        let distinct = self.inner.len();
        let total = self.total_reads();
        Some(table(
            &["Metric", "Value"],
            &[
                vec!["Sampled Reads".into(), total.to_string()],
                vec!["Distinct Sequences".into(), distinct.to_string()],
                vec!["Pct Unique".into(), format!("{:.2}%", pct(distinct, total))],
            ],
        ))
    }

    /// Top overrepresented sequences (most frequent first), capped at
    /// [`OVERREPRESENTED_SUMMARY_LIMIT`] for the summary report.
    fn overrepresented_table(&self, threshold_pct: f64) -> Option<String> {
        let overrepresented = self.overrepresented(threshold_pct);
        if overrepresented.is_empty() {
            return None;
        }
        let rows = overrepresented
            .into_iter()
            .take(OVERREPRESENTED_SUMMARY_LIMIT)
            .map(|(seq, count, pct)| {
                vec![
                    truncate_sequence(seq),
                    count.to_string(),
                    format!("{pct:.3}%"),
                ]
            })
            .collect::<Vec<_>>();
        Some(table(&["Sequence", "Count", "Pct"], &rows))
    }
}

#[derive(Clone)]
pub struct SequenceDuplicationLevels {
    /// Only records with `index() < sample_size` are counted. `0` means
    /// unlimited (every record is considered).
    sample_size: usize,
    /// Whether to write the bucketed duplication-level report.
    emit_levels: bool,
    /// Whether to write the overrepresented-sequences report.
    emit_overrepresented: bool,
    /// Minimum percentage of sampled reads a sequence must represent to be
    /// flagged as overrepresented.
    overrepresented_threshold: f64,

    /// thread - duplication counts (primary)
    t_dup: DuplicationCounter,
    /// thread - duplication counts (extended)
    t_xdup: DuplicationCounter,

    /// global - duplication counts (primary)
    dup: Arc<Mutex<DuplicationCounter>>,
    /// global - duplication counts (extended)
    xdup: Arc<Mutex<DuplicationCounter>>,
}
impl Default for SequenceDuplicationLevels {
    fn default() -> Self {
        Self::new(
            DEFAULT_DUP_SAMPLE_SIZE,
            true,
            true,
            DEFAULT_OVERREPRESENTED_THRESHOLD_PCT,
        )
    }
}
impl SequenceDuplicationLevels {
    /// Both `emit_levels` and `emit_overrepresented` read from the same
    /// underlying per-sequence counts, so this module only needs
    /// constructing once even when both reports are wanted.
    pub fn new(
        sample_size: usize,
        emit_levels: bool,
        emit_overrepresented: bool,
        overrepresented_threshold: f64,
    ) -> Self {
        Self {
            sample_size,
            emit_levels,
            emit_overrepresented,
            overrepresented_threshold,
            t_dup: DuplicationCounter::default(),
            t_xdup: DuplicationCounter::default(),
            dup: Arc::default(),
            xdup: Arc::default(),
        }
    }
}
impl QcModule for SequenceDuplicationLevels {
    fn push<R: BinseqRecord>(&mut self, record: &R) {
        if self.sample_size > 0 && record.index() as usize >= self.sample_size {
            return;
        }
        self.t_dup.push(record.sseq());
        if record.is_paired() {
            self.t_xdup.push(record.xseq());
        }
    }

    fn sync_final(&mut self) {
        self.dup.lock().ingest(&mut self.t_dup);
        self.xdup.lock().ingest(&mut self.t_xdup);
    }

    fn finish<P: AsRef<Path>>(&mut self, outdir: P) -> Result<()> {
        if !outdir.as_ref().exists() {
            make_directory(outdir.as_ref())?;
        }

        let write_to = |counter: &DuplicationCounter,
                        dup_path: &str,
                        overrep_path: &str,
                        label: &str|
         -> Result<()> {
            if counter.is_empty() {
                return Ok(());
            }
            if self.emit_levels {
                let mut handle = match_output(Some(outdir.as_ref().join(dup_path)))?;
                counter.serialize_levels_to(&mut handle)?;
            }
            if self.emit_overrepresented {
                if counter
                    .overrepresented(self.overrepresented_threshold)
                    .is_empty()
                {
                    trace!(
                        "No {label} sequences met the overrepresented threshold ({}%)",
                        self.overrepresented_threshold
                    );
                } else {
                    let mut handle = match_output(Some(outdir.as_ref().join(overrep_path)))?;
                    counter.serialize_overrepresented_to(
                        &mut handle,
                        self.overrepresented_threshold,
                    )?;
                }
            }
            Ok(())
        };

        write_to(
            &self.dup.lock(),
            DUPLICATION_LEVELS_PRIMARY_PATH,
            OVERREPRESENTED_PRIMARY_PATH,
            "R1",
        )?;
        write_to(
            &self.xdup.lock(),
            DUPLICATION_LEVELS_EXTENDED_PATH,
            OVERREPRESENTED_EXTENDED_PATH,
            "R2",
        )?;

        Ok(())
    }

    fn summarize(&self) -> String {
        let mut out = String::new();

        if self.emit_levels {
            let primary = self.dup.lock().summary_table();
            let extended = self.xdup.lock().summary_table();
            out.push_str(&dual_section(
                "Sequence Duplication Levels",
                primary,
                extended,
            ));
        }

        if self.emit_overrepresented {
            let primary = self
                .dup
                .lock()
                .overrepresented_table(self.overrepresented_threshold);
            let extended = self
                .xdup
                .lock()
                .overrepresented_table(self.overrepresented_threshold);
            let section = dual_section("Overrepresented Sequences", primary, extended);
            if !section.is_empty() {
                if !out.is_empty() {
                    out.push('\n');
                }
                out.push_str(&section);
            }
        }

        out
    }
}
