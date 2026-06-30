use super::QcModule;
use crate::cli::QcOptions;

#[derive(Clone, Copy)]
pub struct QcConfig {
    per_base_qual: bool,
    per_seq_qual: bool,
}
impl QcConfig {
    pub fn from_opts(opts: &QcOptions) -> Self {
        Self {
            per_base_qual: !opts.skip_base_qual,
            per_seq_qual: !opts.skip_seq_qual,
        }
    }

    pub fn build_qc_modules(&self) -> Vec<QcModule> {
        let mut modules = Vec::default();
        if self.per_base_qual {
            modules.push(QcModule::PerBaseQual(Default::default()))
        }
        if self.per_seq_qual {
            modules.push(QcModule::PerSeqQual(Default::default()))
        }
        modules
    }
}
