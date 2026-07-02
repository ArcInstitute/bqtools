use std::{io::Write, path::Path, sync::Arc};

use anyhow::Result;
use binseq::BinseqRecord;
use parking_lot::Mutex;
use serde::Serialize;

use super::{QualAbundance, DEFAULT_QUAL_ABUNDANCE, PHRED_OFFSET};
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
}
