use std::path::{Path, PathBuf};

use super::{base_quality::PerBaseSequenceQuality, seq_quality::PerSequenceQuality, QcConfig};

use anyhow::Result;
use binseq::ParallelProcessor;

/// TODO: per base sequence quality
/// TODO: per sequence quality
/// TODO: per base sequence content
/// TODO: per sequence GC content
/// TODO: per base N content
/// TODO: sequence length distribution
/// TODO: sequence duplication levels
/// TODO: overrepresented sequences
/// TODO: adapter content
#[derive(Clone, Default)]
pub struct QcProcessor {
    outdir: PathBuf,
    bsq: Option<PerBaseSequenceQuality>,
    sq: Option<PerSequenceQuality>,
}
impl QcProcessor {
    pub fn new<P: AsRef<Path>>(outdir: P, config: QcConfig) -> Self {
        Self {
            outdir: outdir.as_ref().to_path_buf(),
            bsq: config.per_base_qual.then(|| Default::default()),
            sq: config.per_seq_qual.then(|| Default::default()),
        }
    }

    pub fn finish(&mut self) -> Result<()> {
        if let Some(ref mut bsq) = self.bsq {
            bsq.write_to_outdir(&self.outdir)?;
        }
        if let Some(ref mut sq) = self.sq {
            sq.write_to_outdir(&self.outdir)?;
        }
        Ok(())
    }
}
impl ParallelProcessor for QcProcessor {
    fn process_record<R: binseq::prelude::BinseqRecord>(
        &mut self,
        record: R,
    ) -> binseq::Result<()> {
        if let Some(ref mut bsq) = self.bsq {
            bsq.push(&record);
        }
        if let Some(ref mut sq) = self.sq {
            sq.push(&record);
        }
        Ok(())
    }

    fn on_batch_complete(&mut self) -> binseq::Result<()> {
        if let Some(ref mut bsq) = self.bsq {
            bsq.sync()
        }
        if let Some(ref mut sq) = self.sq {
            sq.sync()
        }
        Ok(())
    }
}
