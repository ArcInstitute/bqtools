use std::{any::type_name, path::Path};

use anyhow::Result;
use binseq::BinseqRecord;

use crate::commands::qc::{
    base_content::PerBaseSequenceContent, base_quality::PerBaseSequenceQuality,
    gc_content::PerSequenceGcContent, seq_length::SequenceLengthDistribution,
    seq_quality::PerSequenceQuality,
};

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
    Bc(PerBaseSequenceContent),
    Gc(PerSequenceGcContent),
    Sl(SequenceLengthDistribution),
}
impl QcModuleType {
    pub fn new_bsq() -> Self {
        Self::Bsq(Default::default())
    }
    pub fn new_sq() -> Self {
        Self::Sq(Default::default())
    }
    pub fn new_bc() -> Self {
        Self::Bc(Default::default())
    }
    pub fn new_gc() -> Self {
        Self::Gc(Default::default())
    }
    pub fn new_sl() -> Self {
        Self::Sl(Default::default())
    }
}
impl QcModule for QcModuleType {
    fn desc(&self) -> &'static str {
        match self {
            Self::Bsq(x) => x.desc(),
            Self::Sq(x) => x.desc(),
            Self::Bc(x) => x.desc(),
            Self::Gc(x) => x.desc(),
            Self::Sl(x) => x.desc(),
        }
    }
    fn push<R: BinseqRecord>(&mut self, record: &R) {
        match self {
            Self::Bsq(x) => x.push(record),
            Self::Sq(x) => x.push(record),
            Self::Bc(x) => x.push(record),
            Self::Gc(x) => x.push(record),
            Self::Sl(x) => x.push(record),
        }
    }
    fn sync(&mut self) {
        match self {
            Self::Bsq(x) => x.sync(),
            Self::Sq(x) => x.sync(),
            Self::Bc(x) => x.sync(),
            Self::Gc(x) => x.sync(),
            Self::Sl(x) => x.sync(),
        }
    }
    fn finish<P: AsRef<Path>>(&mut self, outdir: P) -> Result<()> {
        match self {
            Self::Bsq(x) => x.finish(&outdir),
            Self::Sq(x) => x.finish(&outdir),
            Self::Bc(x) => x.finish(&outdir),
            Self::Gc(x) => x.finish(&outdir),
            Self::Sl(x) => x.finish(&outdir),
        }
    }
}
