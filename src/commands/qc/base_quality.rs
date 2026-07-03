use std::{io::Write, path::Path, sync::Arc};

use anyhow::Result;
use binseq::BinseqRecord;
use parking_lot::Mutex;
use serde::Serialize;

use super::{report::table, QualAbundance, DEFAULT_QUAL_ABUNDANCE, PHRED_OFFSET};
use crate::commands::{match_output, qc::modules::QcModule, utils::make_directory};

const BASE_QUALITY_PRIMARY_PATH: &str = "base_quality_R1.tsv";
const BASE_QUALITY_EXTENDED_PATH: &str = "base_quality_R2.tsv";

#[derive(Serialize)]
pub struct BaseQualityRecord {
    pos: usize,
    qual: usize,
    count: usize,
}

#[derive(Debug, Clone, Default)]
pub struct BaseHistogram {
    /// Outer: position
    /// Inner: quality
    inner: Vec<QualAbundance>,
}
impl BaseHistogram {
    /// Checks if empty
    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
    /// Number of positions tracked
    fn len(&self) -> usize {
        self.inner.len()
    }
    /// Track quality score histogram over positions
    fn push(&mut self, qual: &[u8]) {
        if qual.is_empty() {
            return;
        }
        if self.inner.len() <= qual.len() {
            self.inner.resize(qual.len(), DEFAULT_QUAL_ABUNDANCE);
        }
        qual.iter()
            .map(|q| q.saturating_sub(PHRED_OFFSET) as usize)
            .zip(self.inner.iter_mut())
            .for_each(|(q, pos_vec)| {
                pos_vec[q.min(pos_vec.len() - 1)] += 1;
            });
    }
    fn ingest(&mut self, other: &mut Self) {
        if self.len() < other.len() {
            self.inner.resize(other.len(), DEFAULT_QUAL_ABUNDANCE);
        }
        self.inner
            .iter_mut()
            .zip(other.inner.iter_mut())
            .for_each(|(self_pos, other_pos)| {
                self_pos
                    .iter_mut()
                    .zip(other_pos.iter_mut())
                    .for_each(|(self_q, other_q)| {
                        *self_q += *other_q;
                        *other_q = 0;
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
            .try_for_each(|(pos, inner)| -> Result<()> {
                inner
                    .iter()
                    .enumerate()
                    .filter(|(_, count)| **count > 0)
                    .map(|(qual, count)| (qual, *count))
                    .try_for_each(|(qual, count)| -> Result<()> {
                        ser.serialize(&BaseQualityRecord { pos, qual, count })
                            .map_err(Into::into)
                    })
            })?;

        ser.flush().map_err(Into::into)
    }

    /// Mean quality score at each position.
    fn position_means(&self) -> Vec<f64> {
        self.inner
            .iter()
            .map(|counts| {
                let total: usize = counts.iter().sum();
                if total == 0 {
                    0.0
                } else {
                    let sum: usize = counts.iter().enumerate().map(|(q, &c)| q * c).sum();
                    sum as f64 / total as f64
                }
            })
            .collect()
    }

    fn summary_table(&self) -> Option<String> {
        if self.is_empty() {
            return None;
        }

        let mut num = 0usize;
        let mut den = 0usize;
        for counts in &self.inner {
            for (q, &c) in counts.iter().enumerate() {
                num += q * c;
                den += c;
            }
        }
        let overall_mean = if den == 0 {
            0.0
        } else {
            num as f64 / den as f64
        };

        let means = self.position_means();
        let (min_pos, min_mean) = means
            .iter()
            .copied()
            .enumerate()
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap_or((0, 0.0));
        let (max_pos, max_mean) = means
            .iter()
            .copied()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap_or((0, 0.0));

        Some(table(
            &["Metric", "Value"],
            &[
                vec!["Positions".into(), means.len().to_string()],
                vec!["Mean Quality".into(), format!("{overall_mean:.2}")],
                vec![
                    "Lowest Mean Quality".into(),
                    format!("{min_mean:.2} (pos {min_pos})"),
                ],
                vec![
                    "Highest Mean Quality".into(),
                    format!("{max_mean:.2} (pos {max_pos})"),
                ],
            ],
        ))
    }
}

#[derive(Clone, Default)]
pub struct PerBaseSequenceQuality {
    /// thread - per base sequence quality (primary)
    t_base_squal: BaseHistogram,
    /// thread - per base sequence quality (extended)
    t_base_xqual: BaseHistogram,
    /// thread - number of records
    t_n_records: usize,

    /// global - per base sequence quality (primary)
    base_squal: Arc<Mutex<BaseHistogram>>,
    /// global - per base sequence quality (extended)
    base_xqual: Arc<Mutex<BaseHistogram>>,
    /// global - number of records
    n_records: Arc<Mutex<usize>>,
}
impl QcModule for PerBaseSequenceQuality {
    fn push<R: BinseqRecord>(&mut self, record: &R) {
        self.t_base_squal.push(record.squal());
        self.t_base_xqual.push(record.xqual());
        self.t_n_records += 1;
    }

    fn sync_final(&mut self) {
        self.base_squal.lock().ingest(&mut self.t_base_squal);
        self.base_xqual.lock().ingest(&mut self.t_base_xqual);

        // handle total
        *self.n_records.lock() += self.t_n_records;
        self.t_n_records = 0;
    }

    fn finish<P: AsRef<Path>>(&mut self, outdir: P) -> Result<()> {
        if !outdir.as_ref().exists() {
            make_directory(outdir.as_ref())?;
        }

        let write_to = |base_qual: &BaseHistogram, primary: bool| -> Result<()> {
            if base_qual.is_empty() {
                return Ok(());
            }
            let mut handle = if primary {
                match_output(Some(outdir.as_ref().join(BASE_QUALITY_PRIMARY_PATH)))
            } else {
                match_output(Some(outdir.as_ref().join(BASE_QUALITY_EXTENDED_PATH)))
            }?;
            base_qual.serialize_to(&mut handle)
        };

        write_to(&self.base_squal.lock(), true)?;
        write_to(&self.base_xqual.lock(), false)?;

        Ok(())
    }

    fn summarize(&self) -> String {
        let primary = self.base_squal.lock().summary_table();
        let extended = self.base_xqual.lock().summary_table();
        super::report::dual_section("Per-Base Sequence Quality", primary, extended)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn phred(scores: &[u8]) -> Vec<u8> {
        scores.iter().map(|&s| s + PHRED_OFFSET).collect()
    }

    #[test]
    fn starts_empty() {
        assert!(BaseHistogram::default().is_empty());
    }

    #[test]
    fn push_ignores_empty_quality() {
        let mut hist = BaseHistogram::default();
        hist.push(&[]);
        assert!(hist.is_empty());
    }

    #[test]
    fn push_tracks_per_position_quality() {
        let mut hist = BaseHistogram::default();
        hist.push(&phred(&[10, 20, 30]));
        assert!(!hist.is_empty());
        assert_eq!(hist.len(), 3);
        assert_eq!(hist.position_means(), vec![10.0, 20.0, 30.0]);
    }

    #[test]
    fn push_accumulates_across_reads() {
        let mut hist = BaseHistogram::default();
        hist.push(&phred(&[10, 10]));
        hist.push(&phred(&[30, 30]));
        assert_eq!(hist.position_means(), vec![20.0, 20.0]);
    }

    #[test]
    fn summary_table_none_when_empty() {
        assert!(BaseHistogram::default().summary_table().is_none());
    }

    #[test]
    fn summary_table_reports_overall_and_extreme_positions() {
        let mut hist = BaseHistogram::default();
        hist.push(&phred(&[10, 40]));
        let summary = hist.summary_table().expect("non-empty histogram");
        assert!(summary.contains("| Positions | 2 |"));
        assert!(summary.contains("| Mean Quality | 25.00 |"));
        assert!(summary.contains("| Lowest Mean Quality | 10.00 (pos 0) |"));
        assert!(summary.contains("| Highest Mean Quality | 40.00 (pos 1) |"));
    }

    #[test]
    fn ingest_merges_counts_and_zeroes_source() {
        let mut a = BaseHistogram::default();
        let mut b = BaseHistogram::default();
        a.push(&phred(&[10, 10]));
        b.push(&phred(&[30, 30]));

        a.ingest(&mut b);

        assert_eq!(a.position_means(), vec![20.0, 20.0]);
        assert_eq!(b.position_means(), vec![0.0, 0.0]);
    }
}
