mod cli;
mod decode;
mod encode;
mod formats;
mod input;
mod output;

pub use cli::{Cli, Commands};
pub use decode::DecodeCommand;
pub use encode::EncodeCommand;
pub use formats::FileFormat;
pub use input::{InputBinseq, InputFile};
pub use output::{Mate, OutputBinseq, OutputFile};
