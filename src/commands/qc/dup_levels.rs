use std::{io::Write, path::Path, sync::Arc};

use anyhow::Result;
use binseq::BinseqRecord;
use hashbrown::HashMap;
use parking_lot::Mutex;
use serde::Serialize;

use crate::commands::{match_output, qc::modules::QcModule, utils::make_directory};

const DUP_PRIMARY_PATH: &str = "dup_R1.tsv";
const DUP_EXTENDED_PATH: &str = "dup_R2.tsv";

/// Number of leading records (by global file index) considered for
/// duplication analysis. Bounding this keeps memory flat regardless of file
/// size - mirrors `FastQC`'s own subsampling behavior for this module.
pub const DEFAULT_DUP_SAMPLE_SIZE: usize = 100_000;

/// FastQC-style duplication level buckets: exact counts 1-9, then cumulative
/// thresholds beyond that.
const LEVELS: &[usize] = &[
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 50, 100, 500, 1000, 5000, 10000,
];
const LABELS: &[&str] = &[
    "1", "2", "3", "4", "5", "6", "7", "8", "9", ">10", ">50", ">100", ">500", ">1k", ">5k", ">10k",
];

fn pct(n: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        (n as f64 / total as f64) * 100.0
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
    fn serialize_to<W: Write>(&self, wtr: &mut W) -> Result<()> {
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
}

#[derive(Clone)]
pub struct SequenceDuplicationLevels {
    /// Only records with `index() < sample_size` are counted. `0` means
    /// unlimited (every record is considered).
    sample_size: usize,

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
        Self::with_sample_size(DEFAULT_DUP_SAMPLE_SIZE)
    }
}
impl SequenceDuplicationLevels {
    pub fn with_sample_size(sample_size: usize) -> Self {
        Self {
            sample_size,
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

        let write_to = |counter: &DuplicationCounter, primary: bool| -> Result<()> {
            if counter.is_empty() {
                return Ok(());
            }
            let mut handle = if primary {
                match_output(Some(outdir.as_ref().join(DUP_PRIMARY_PATH)))
            } else {
                match_output(Some(outdir.as_ref().join(DUP_EXTENDED_PATH)))
            }?;
            counter.serialize_to(&mut handle)
        };

        write_to(&self.dup.lock(), true)?;
        write_to(&self.xdup.lock(), false)?;

        Ok(())
    }
}
