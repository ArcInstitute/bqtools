pub mod cat;
pub mod count;
pub mod decode;
pub mod encode;
pub mod grep;
pub mod index;
pub mod sample;
mod utils;

pub use utils::{compress_gzip_passthrough, compress_zstd_passthrough, match_input, match_output};
