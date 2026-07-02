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
    BaseQuality(PerBaseSequenceQuality),
    SeqQuality(PerSequenceQuality),
    BaseContent(PerBaseSequenceContent),
    GcContent(PerSequenceGcContent),
    SeqLength(SequenceLengthDistribution),
    Duplication(SequenceDuplicationLevels),
}
impl QcModuleType {
    pub fn new_base_quality() -> Self {
        Self::BaseQuality(PerBaseSequenceQuality::default())
    }
    pub fn new_seq_quality() -> Self {
        Self::SeqQuality(PerSequenceQuality::default())
    }
    pub fn new_base_content() -> Self {
        Self::BaseContent(PerBaseSequenceContent::default())
    }
    pub fn new_gc_content() -> Self {
        Self::GcContent(PerSequenceGcContent::default())
    }
    pub fn new_seq_length() -> Self {
        Self::SeqLength(SequenceLengthDistribution::default())
    }
    pub fn new_duplication(
        sample_size: usize,
        emit_levels: bool,
        emit_overrepresented: bool,
        overrepresented_threshold: f64,
    ) -> Self {
        Self::Duplication(SequenceDuplicationLevels::new(
            sample_size,
            emit_levels,
            emit_overrepresented,
            overrepresented_threshold,
        ))
    }
}
impl QcModule for QcModuleType {
    fn desc(&self) -> &'static str {
        match self {
            Self::BaseQuality(x) => x.desc(),
            Self::SeqQuality(x) => x.desc(),
            Self::BaseContent(x) => x.desc(),
            Self::GcContent(x) => x.desc(),
            Self::SeqLength(x) => x.desc(),
            Self::Duplication(x) => x.desc(),
        }
    }
    fn push<R: BinseqRecord>(&mut self, record: &R) {
        match self {
            Self::BaseQuality(x) => x.push(record),
            Self::SeqQuality(x) => x.push(record),
            Self::BaseContent(x) => x.push(record),
            Self::GcContent(x) => x.push(record),
            Self::SeqLength(x) => x.push(record),
            Self::Duplication(x) => x.push(record),
        }
    }
    fn sync_batch(&mut self) {
        match self {
            Self::BaseQuality(x) => x.sync_batch(),
            Self::SeqQuality(x) => x.sync_batch(),
            Self::BaseContent(x) => x.sync_batch(),
            Self::GcContent(x) => x.sync_batch(),
            Self::SeqLength(x) => x.sync_batch(),
            Self::Duplication(x) => x.sync_batch(),
        }
    }
    fn sync_final(&mut self) {
        match self {
            Self::BaseQuality(x) => x.sync_final(),
            Self::SeqQuality(x) => x.sync_final(),
            Self::BaseContent(x) => x.sync_final(),
            Self::GcContent(x) => x.sync_final(),
            Self::SeqLength(x) => x.sync_final(),
            Self::Duplication(x) => x.sync_final(),
        }
    }
    fn finish<P: AsRef<Path>>(&mut self, outdir: P) -> Result<()> {
        match self {
            Self::BaseQuality(x) => x.finish(&outdir),
            Self::SeqQuality(x) => x.finish(&outdir),
            Self::BaseContent(x) => x.finish(&outdir),
            Self::GcContent(x) => x.finish(&outdir),
            Self::SeqLength(x) => x.finish(&outdir),
            Self::Duplication(x) => x.finish(&outdir),
        }
    }
}
