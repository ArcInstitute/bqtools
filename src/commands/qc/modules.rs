use std::{any::type_name, path::Path};

use anyhow::Result;
use binseq::BinseqRecord;

use crate::commands::qc::{
    base_content::PerBaseSequenceContent, base_quality::PerBaseSequenceQuality,
    dup_levels::SequenceDuplicationLevels, gc_content::PerSequenceGcContent,
    seq_length::SequenceLengthDistribution, seq_quality::PerSequenceQuality,
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

    /// Called after each batch. Default no-op.
    ///
    /// Use if on-thread memory accumulation should be drained at regular
    /// intervals.
    fn sync_batch(&mut self) {}

    /// Called once, when a thread has finished all of its batches. This is
    /// where thread-local state should be merged into shared/global state.
    fn sync_final(&mut self);

    fn finish<P: AsRef<Path>>(&mut self, outdir: P) -> Result<()>;
}

#[derive(Clone)]
pub enum QcModuleType {
    Bsq(PerBaseSequenceQuality),
    Sq(PerSequenceQuality),
    Bc(PerBaseSequenceContent),
    Gc(PerSequenceGcContent),
    Sl(SequenceLengthDistribution),
    Dup(SequenceDuplicationLevels),
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
    pub fn new_dup(sample_size: usize) -> Self {
        Self::Dup(SequenceDuplicationLevels::with_sample_size(sample_size))
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
            Self::Dup(x) => x.desc(),
        }
    }
    fn push<R: BinseqRecord>(&mut self, record: &R) {
        match self {
            Self::Bsq(x) => x.push(record),
            Self::Sq(x) => x.push(record),
            Self::Bc(x) => x.push(record),
            Self::Gc(x) => x.push(record),
            Self::Sl(x) => x.push(record),
            Self::Dup(x) => x.push(record),
        }
    }
    fn sync_batch(&mut self) {
        match self {
            Self::Bsq(x) => x.sync_batch(),
            Self::Sq(x) => x.sync_batch(),
            Self::Bc(x) => x.sync_batch(),
            Self::Gc(x) => x.sync_batch(),
            Self::Sl(x) => x.sync_batch(),
            Self::Dup(x) => x.sync_batch(),
        }
    }
    fn sync_final(&mut self) {
        match self {
            Self::Bsq(x) => x.sync_final(),
            Self::Sq(x) => x.sync_final(),
            Self::Bc(x) => x.sync_final(),
            Self::Gc(x) => x.sync_final(),
            Self::Sl(x) => x.sync_final(),
            Self::Dup(x) => x.sync_final(),
        }
    }
    fn finish<P: AsRef<Path>>(&mut self, outdir: P) -> Result<()> {
        match self {
            Self::Bsq(x) => x.finish(&outdir),
            Self::Sq(x) => x.finish(&outdir),
            Self::Bc(x) => x.finish(&outdir),
            Self::Gc(x) => x.finish(&outdir),
            Self::Sl(x) => x.finish(&outdir),
            Self::Dup(x) => x.finish(&outdir),
        }
    }
}
