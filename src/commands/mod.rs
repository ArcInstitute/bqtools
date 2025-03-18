pub mod cat;
pub mod count;
pub mod decode;
pub mod encode;
pub mod index;
mod utils;

pub use utils::{compress_gzip_passthrough, compress_zstd_passthrough, match_input, match_output};
