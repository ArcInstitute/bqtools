use log::trace;

use crate::{
    cli::QcOptions,
    commands::qc::modules::{QcModule, QcModuleType},
};

// Mirrors QcOptions: each bool independently enables one QC module, not a
// state machine - see the comment on QcOptions in src/cli/qc.rs.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy)]
pub struct QcConfig {
    per_base_qual: bool,
    per_seq_qual: bool,
    per_base_content: bool,
    per_seq_gc: bool,
    seq_length: bool,
    dup_levels: bool,
    overrepresented: bool,
    dup_sample_size: usize,
    overrepresented_threshold: f64,
}
impl QcConfig {
    pub fn from_opts(opts: &QcOptions) -> Self {
        Self {
            per_base_qual: !opts.skip_base_qual,
            per_seq_qual: !opts.skip_seq_qual,
            per_base_content: !opts.skip_base_content,
            per_seq_gc: !opts.skip_seq_gc,
            seq_length: !opts.skip_seq_length,
            dup_levels: !opts.skip_dup_levels,
            overrepresented: !opts.skip_overrepresented,
            dup_sample_size: opts.dup_sample_size,
            overrepresented_threshold: opts.overrepresented_threshold,
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
            .then(|| add_module(QcModuleType::new_base_quality()));
        self.per_seq_qual
            .then(|| add_module(QcModuleType::new_seq_quality()));
        self.per_base_content
            .then(|| add_module(QcModuleType::new_base_content()));
        self.per_seq_gc
            .then(|| add_module(QcModuleType::new_gc_content()));
        self.seq_length
            .then(|| add_module(QcModuleType::new_seq_length()));
        if self.dup_levels || self.overrepresented {
            add_module(QcModuleType::new_duplication(
                self.dup_sample_size,
                self.dup_levels,
                self.overrepresented,
                self.overrepresented_threshold,
            ));
        }
        trace!("{} modules loaded", modules.len());
        modules
    }
}
