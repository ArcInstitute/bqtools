mod cat;
mod cli;
mod count;
mod decode;
mod encode;
mod formats;
mod grep;
mod index;
mod input;
mod output;
mod sample;

pub use cat::CatCommand;
pub use cli::{Cli, Commands};
pub use count::CountCommand;
pub use decode::DecodeCommand;
pub use encode::EncodeCommand;
pub use formats::FileFormat;
pub use grep::GrepCommand;
pub use index::IndexCommand;
pub use input::{InputBinseq, InputFile, MultiInputBinseq};
pub use output::{
    BinseqMode, Mate, OutputBinseq, OutputFile, PadMode, TruncateConfig, TruncateMate, TruncateMode,
};
pub use sample::SampleCommand;
