pub mod decode;
pub mod encode;
mod utils;

pub use utils::{
    compress_gzip_passthrough, compress_zstd_passthrough, decompress_zstd_passthrough, match_input,
    match_output, reopen_output,
};
