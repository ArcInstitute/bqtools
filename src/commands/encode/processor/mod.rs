mod bq;
mod cbq;
mod vbq;

pub use bq::BqEncoder;
pub use vbq::VbqEncoder;

/// Default capacity for the buffer used by the processor.
const DEFAULT_CAPACITY: usize = 128 * 1024;

/// Default debug interval for logging progress
const DEBUG_INTERVAL: usize = 1024;
