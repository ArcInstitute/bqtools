use std::path::PathBuf;

use anyhow::{bail, Result};
use clap::Parser;

use super::{BinseqMode, FileFormat};

#[derive(Parser, Debug, Clone)]
#[clap(next_help_heading = "INPUT FILE OPTIONS")]
pub struct InputFile {
    /// Input file [default: stdin]
    ///
    /// Can specify either zero (stdin), one, or two (paired) input files.
    #[clap(help = "Input file [default: stdin]", num_args = 0..=2)]
    pub input: Vec<String>,

    #[clap(short, long, help = "Input file format")]
    pub format: Option<FileFormat>,

    /// Batch size (in records) to use in parallel processing
    ///
    /// Set this to a lower value for embedding genomes to better
    /// make use of parallelism (e.g. 2-4).
    #[clap(short, long)]
    pub batch_size: Option<usize>,

    #[clap(short = 'I', long, help = "Interleaved input file format")]
    pub interleaved: bool,

    /// Apply encoding to all fasta/fastq files in the provided directory input.
    ///
    /// For R1/R2 encodings pair this with the `--paired` option.
    ///
    /// Options used will be applied to all in the directory.
    #[clap(short = 'r', long, requires = "mode")]
    pub recursive: bool,
}
impl InputFile {
    pub fn single_path(&self) -> Result<Option<&str>> {
        match self.input.len() {
            0 => Ok(None),
            1 => Ok(Some(&self.input[0])),
            _ => bail!("Requested single input file, but multiple files were provided."),
        }
    }

    pub fn paired_paths(&self) -> Result<(&str, &str)> {
        match self.input.len() {
            2 => Ok((&self.input[0], &self.input[1])),
            _ => bail!("Two input files are required."),
        }
    }

    pub fn paired(&self) -> bool {
        self.input.len() == 2
    }

    pub fn as_directory(&self) -> Result<PathBuf> {
        if !self.recursive {
            bail!("Recursive mode is required to process a directory.");
        }
        let path = PathBuf::from(&self.input[0]);
        if !path.is_dir() {
            bail!("Input path is not a directory: {}", path.display());
        }
        Ok(path)
    }
}

#[derive(Parser, Debug)]
#[clap(next_help_heading = "INPUT FILE OPTIONS")]
pub struct InputBinseq {
    #[clap(help = "Input binseq file")]
    pub input: String,
}
impl InputBinseq {
    pub fn path(&self) -> &str {
        &self.input
    }

    pub fn mode(&self) -> Result<BinseqMode> {
        BinseqMode::determine(self.path())
    }
}

#[derive(Parser, Debug)]
#[clap(next_help_heading = "INPUT FILE OPTIONS")]
pub struct MultiInputBinseq {
    /// Input binseq files
    #[clap(num_args = 1..)]
    pub input: Vec<String>,
}
