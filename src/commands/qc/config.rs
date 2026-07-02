use log::trace;

use crate::{
    cli::QcOptions,
    commands::qc::modules::{QcModule, QcModuleType},
};

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

    pub fn build_qc_modules(&self) -> Vec<QcModuleType> {
        let mut modules = Vec::default();

        let mut add_module = |module: QcModuleType| {
            trace!("Loaded: {}", module.desc());
            modules.push(module);
        };

        trace!("Loading QC modules...");
        if self.per_base_qual {
            add_module(QcModuleType::new_bsq());
        }
        if self.per_seq_qual {
            add_module(QcModuleType::new_sq());
        }
        trace!("Modules loaded.");
        modules
    }
}
