pub mod cat;
pub mod count;
pub mod decode;
pub mod encode;
pub mod grep;
pub mod sample;
mod utils;

pub use utils::{compress_passthrough, match_output, CompressionType};
