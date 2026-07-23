pub mod cat;
pub mod decode;
pub mod encode;
pub mod grep;
pub mod info;
pub mod pipe;
pub mod qc;
pub mod revcomp;
pub mod sample;
pub mod split;
mod utils;
pub mod verify;

pub use utils::{compress_passthrough, match_output, CompressionType};
