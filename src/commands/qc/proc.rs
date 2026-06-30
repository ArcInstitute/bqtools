use std::path::{Path, PathBuf};

use super::{QcConfig, QcModule};

use anyhow::{bail, Result};
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
    modules: Vec<QcModule>,
}
impl QcProcessor {
    pub fn new<P: AsRef<Path>>(outdir: P, config: QcConfig) -> Result<Self> {
        let modules = config.build_qc_modules();
        if modules.is_empty() {
            bail!("Must provide at least one QC module to process")
        }
        Ok(Self {
            outdir: outdir.as_ref().to_path_buf(),
            modules,
        })
    }

    pub fn finish(&mut self) -> Result<()> {
        self.modules
            .iter_mut()
            .try_for_each(|m| m.finish(&self.outdir))
            .map_err(Into::into)
    }
}
impl ParallelProcessor for QcProcessor {
    fn process_record<R: binseq::prelude::BinseqRecord>(
        &mut self,
        record: R,
    ) -> binseq::Result<()> {
        self.modules.iter_mut().for_each(|m| m.push(&record));
        Ok(())
    }

    fn on_batch_complete(&mut self) -> binseq::Result<()> {
        self.modules.iter_mut().for_each(|m| m.sync());
        Ok(())
    }
}
