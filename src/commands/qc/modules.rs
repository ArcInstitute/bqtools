use std::{any::type_name, path::Path};

use anyhow::Result;
use binseq::BinseqRecord;

use crate::commands::qc::{base_quality::PerBaseSequenceQuality, seq_quality::PerSequenceQuality};

pub trait QcModule {
    fn desc(&self) -> &'static str {
        let full_type = type_name::<Self>();
        if let Some(base) = full_type.split("::").last() {
            base
        } else {
            full_type
        }
    }
    fn push<R: BinseqRecord>(&mut self, record: &R);
    fn sync(&mut self);
    fn finish<P: AsRef<Path>>(&mut self, outdir: P) -> Result<()>;
}

#[derive(Clone)]
pub enum QcModuleType {
    Bsq(PerBaseSequenceQuality),
    Sq(PerSequenceQuality),
}
impl QcModuleType {
    pub fn new_bsq() -> Self {
        Self::Bsq(Default::default())
    }
    pub fn new_sq() -> Self {
        Self::Sq(Default::default())
    }
}
impl QcModule for QcModuleType {
    fn desc(&self) -> &'static str {
        match self {
            Self::Bsq(x) => x.desc(),
            Self::Sq(x) => x.desc(),
        }
    }
    fn push<R: BinseqRecord>(&mut self, record: &R) {
        match self {
            Self::Bsq(x) => x.push(record),
            Self::Sq(x) => x.push(record),
        }
    }
    fn sync(&mut self) {
        match self {
            Self::Bsq(x) => x.sync(),
            Self::Sq(x) => x.sync(),
        }
    }
    fn finish<P: AsRef<Path>>(&mut self, outdir: P) -> Result<()> {
        match self {
            Self::Bsq(x) => x.finish(&outdir),
            Self::Sq(x) => x.finish(&outdir),
        }
    }
}
