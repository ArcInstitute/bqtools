use log::trace;

use crate::{
    cli::QcOptions,
    commands::qc::modules::{QcModule, QcModuleType},
};

#[derive(Clone, Copy)]
pub struct QcConfig {
    per_base_qual: bool,
    per_seq_qual: bool,
    per_base_content: bool,
    per_seq_gc: bool,
    seq_length: bool,
}
impl QcConfig {
    pub fn from_opts(opts: &QcOptions) -> Self {
        Self {
            per_base_qual: !opts.skip_base_qual,
            per_seq_qual: !opts.skip_seq_qual,
            per_base_content: !opts.skip_base_content,
            per_seq_gc: !opts.skip_seq_gc,
            seq_length: !opts.skip_seq_length,
        }
    }

    pub fn build_qc_modules(&self) -> Vec<QcModuleType> {
        let mut modules = Vec::default();

        let mut add_module = |module: QcModuleType| {
            trace!("Loaded: {}", module.desc());
            modules.push(module);
        };

        trace!("Loading QC modules...");
        self.per_base_qual
            .then(|| add_module(QcModuleType::new_bsq()));
        self.per_seq_qual
            .then(|| add_module(QcModuleType::new_sq()));
        self.per_base_content
            .then(|| add_module(QcModuleType::new_bc()));
        self.per_seq_gc.then(|| add_module(QcModuleType::new_gc()));
        self.seq_length.then(|| add_module(QcModuleType::new_sl()));
        trace!("{} modules loaded", modules.len());
        modules
    }
}
