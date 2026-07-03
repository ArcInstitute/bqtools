use std::{
    io::Write,
    path::{Path, PathBuf},
};

use crate::commands::qc::modules::QcModuleType;

use super::{report::table, QcConfig, QcModule};

use anyhow::{bail, Result};
use binseq::ParallelProcessor;

use crate::commands::{match_output, utils::make_directory};

const SUMMARY_PATH: &str = "summary.md";

/// TODO: adapter content
#[derive(Clone, Default)]
pub struct QcProcessor {
    outdir: PathBuf,
    modules: Vec<QcModuleType>,
    input_path: String,
    num_records: usize,
    paired: bool,
}
impl QcProcessor {
    pub fn new<P: AsRef<Path>>(
        outdir: P,
        config: QcConfig,
        input_path: String,
        num_records: usize,
        paired: bool,
    ) -> Result<Self> {
        let modules = config.build_qc_modules();
        if modules.is_empty() {
            bail!("Must provide at least one QC module to process")
        }
        Ok(Self {
            outdir: outdir.as_ref().to_path_buf(),
            modules,
            input_path,
            num_records,
            paired,
        })
    }

    pub fn finish(&mut self) -> Result<()> {
        self.modules
            .iter_mut()
            .try_for_each(|m| m.finish(&self.outdir))?;
        self.write_summary()
    }

    /// Writes the high-level `summary.md` report: an overview table followed
    /// by each module's headline stats (the full data still lives in each
    /// module's own TSV).
    fn write_summary(&self) -> Result<()> {
        if !self.outdir.exists() {
            make_directory(&self.outdir)?;
        }

        let mut handle = match_output(Some(self.outdir.join(SUMMARY_PATH)))?;

        writeln!(handle, "# BQtools QC Report\n")?;
        write!(
            handle,
            "{}",
            table(
                &["Metric", "Value"],
                &[
                    vec!["Input".into(), self.input_path.clone()],
                    vec!["Reads".into(), self.num_records.to_string()],
                    vec!["Paired".into(), self.paired.to_string()],
                ],
            )
        )?;
        writeln!(handle)?;

        for module in &self.modules {
            let section = module.summarize();
            if !section.is_empty() {
                writeln!(handle, "{section}")?;
            }
        }

        Ok(())
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
        self.modules
            .iter_mut()
            .for_each(super::modules::QcModule::sync_batch);
        Ok(())
    }

    fn on_thread_complete(&mut self) -> binseq::Result<()> {
        self.modules
            .iter_mut()
            .for_each(super::modules::QcModule::sync_final);
        Ok(())
    }
}
