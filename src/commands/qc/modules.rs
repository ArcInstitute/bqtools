use std::path::Path;

use anyhow::Result;
use binseq::BinseqRecord;

use super::base_quality::PerBaseSequenceQuality;
use super::seq_quality::PerSequenceQuality;

#[derive(Clone)]
pub enum QcModule {
    PerBaseQual(PerBaseSequenceQuality),
    PerSeqQual(PerSequenceQuality),
}
impl QcModule {
    pub fn push<R: BinseqRecord>(&mut self, record: &R) {
        match self {
            Self::PerBaseQual(bsq) => bsq.push(record),
            Self::PerSeqQual(sq) => sq.push(record),
        }
    }
    pub fn sync(&mut self) {
        match self {
            Self::PerBaseQual(bsq) => bsq.sync(),
            Self::PerSeqQual(sq) => sq.sync(),
        }
    }
    pub fn finish<P: AsRef<Path>>(&mut self, outdir: P) -> Result<()> {
        match self {
            Self::PerBaseQual(bsq) => bsq.finish(&outdir),
            Self::PerSeqQual(bsq) => bsq.finish(&outdir),
        }
    }
}
