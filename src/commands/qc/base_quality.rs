use std::{io::Write, sync::Arc};

use anyhow::Result;
use binseq::BinseqRecord;
use parking_lot::Mutex;
use serde::Serialize;

const PHRED_OFFSET: u8 = 33;

type QualAbundance = [usize; 94];
const DEFAULT_QUAL_ABUNDANCE: QualAbundance = [0; 94];

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
                    })
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
            pos: usize,
            qual: usize,
            count: usize,
        }

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
                        ser.serialize(&Record { pos, qual, count })
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
impl PerBaseSequenceQuality {
    pub fn pprint<W: Write>(&self, s_wtr: &mut W, x_wtr: Option<&mut W>) -> Result<()> {
        self.base_squal.lock().serialize_to(s_wtr)?;
        if let Some(x_wtr) = x_wtr {
            self.base_xqual.lock().serialize_to(x_wtr)?;
        }
        Ok(())
    }

    /// updates on-thread
    pub fn push<R: BinseqRecord>(&mut self, record: &R) {
        self.t_base_squal.push(record.squal());
        self.t_base_xqual.push(record.xqual());
        self.t_n_records += 1;
    }

    /// syncs on-thread to global
    pub fn sync(&mut self) {
        self.base_squal.lock().ingest(&mut self.t_base_squal);
        self.base_xqual.lock().ingest(&mut self.t_base_xqual);

        // handle total
        *self.n_records.lock() += self.t_n_records;
        self.t_n_records = 0;
    }
}
