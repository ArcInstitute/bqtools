mod cat;
mod cli;
mod decode;
mod encode;
mod formats;
mod grep;
mod info;
mod input;
mod output;
mod pipe;
mod qc;
mod sample;
mod split;

pub use cat::CatCommand;
pub use cli::{Cli, Commands};
pub use decode::DecodeCommand;
pub use encode::EncodeCommand;
pub use formats::FileFormat;
#[cfg(feature = "fuzzy")]
pub use grep::FuzzyArgs;
pub use grep::{GrepCommand, PatternFileArgs};
pub use info::InfoCommand;
pub use input::{InputBinseq, InputFile, MultiInputBinseq};
pub use output::{BinseqConfig, BinseqMode, Mate, OutputBinseq, OutputFile};
pub use pipe::PipeCommand;
pub use qc::{QcCommand, QcOptions};
pub use sample::SampleCommand;
pub use split::SplitCommand;
