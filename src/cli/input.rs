use std::path::PathBuf;

use anyhow::{bail, Result};
use clap::Parser;
use paraseq::fastx;

use crate::types::BoxedReader;

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

    #[clap(flatten)]
    pub recursion: RecursiveOptions,
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

    pub fn build_single_reader(&self) -> Result<fastx::Reader<BoxedReader>> {
        let path = self.single_path()?;
        let reader = load_reader(path, self.batch_size)?;
        Ok(reader)
    }

    pub fn build_paired_readers(
        &self,
    ) -> Result<(fastx::Reader<BoxedReader>, fastx::Reader<BoxedReader>)> {
        let (path1, path2) = self.paired_paths()?;
        let reader1 = load_reader(Some(path1), self.batch_size)?;
        let reader2 = load_reader(Some(path2), self.batch_size)?;
        Ok((reader1, reader2))
    }
}

fn load_reader(
    path: Option<&str>,
    batch_size: Option<usize>,
) -> Result<fastx::Reader<BoxedReader>, paraseq::Error> {
    if let Some(path) = path {
        if path.starts_with("gs://") {
            load_gcs_reader(path, batch_size)
        } else {
            load_simple_reader(Some(path), batch_size)
        }
    } else {
        load_simple_reader(None, batch_size)
    }
}

fn load_simple_reader(
    path: Option<&str>,
    batch_size: Option<usize>,
) -> Result<fastx::Reader<BoxedReader>, paraseq::Error> {
    if let Some(size) = batch_size {
        fastx::Reader::from_optional_path_with_batch_size(path, size)
    } else {
        fastx::Reader::from_optional_path(path)
    }
}

fn load_gcs_reader(
    path: &str,
    batch_size: Option<usize>,
) -> Result<fastx::Reader<BoxedReader>, paraseq::Error> {
    if let Some(size) = batch_size {
        fastx::Reader::from_gcs_with_batch_size(path, size)
    } else {
        fastx::Reader::from_gcs(path)
    }
}

#[derive(Parser, Debug, Clone, PartialEq, Eq)]
#[clap(next_help_heading = "RECURSION OPTIONS")]
pub struct RecursiveOptions {
    /// Encode *{_R1,_R2}* record pairs. Requires `--recursive`.
    #[clap(short = 'R', long = "paired", requires = "recursive")]
    pub paired: bool,

    /// Maximum depth in the directory tree to process. Leaving this option empty will set no limit.
    #[clap(long, requires = "recursive")]
    pub depth: Option<usize>,
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
