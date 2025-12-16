use std::path::PathBuf;

use anyhow::{bail, Result};
use clap::Parser;
use log::debug;
use paraseq::fastx;

#[cfg(not(feature = "gcs"))]
use log::error;

use crate::types::BoxedReader;

use super::FileFormat;

#[derive(Parser, Debug, Clone)]
#[clap(next_help_heading = "INPUT FILE OPTIONS")]
pub struct InputFile {
    /// Input file [default: stdin]
    ///
    /// Can specify either zero (stdin), one, or two (paired) input files.
    ///
    /// If more than two files are provided they will be collated into a single collection.
    /// Use the `--paired` option to specify paired-end input (number of files must be even).
    #[clap(help = "Input file [default: stdin]", num_args = 0..)]
    pub input: Vec<String>,

    #[clap(short, long, help = "Input file format")]
    format: Option<FileFormat>,

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
    #[clap(short = 'r', long)]
    pub recursive: bool,

    /// Path to a text file containing a list of input files to process.
    ///
    /// for R1/R2 encodings pair this with the `--paired` option.
    ///
    /// Options used will be applied to all files in the manifest.
    #[clap(short = 'M', long)]
    pub manifest: Option<String>,

    #[clap(flatten)]
    pub recursion: RecursiveOptions,

    #[clap(flatten)]
    pub batch_encoding_options: BatchEncodingOptions,
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
        self.input.len() == 2 || self.batch_encoding_options.paired
    }

    /// Returns the number of input files.
    pub fn num_files(&self) -> usize {
        self.input.len()
    }

    pub fn format(&self) -> Option<FileFormat> {
        if let Some(format) = self.format {
            Some(format)
        } else if self.input.len() == 1 {
            let path = &self.input[0];
            if path.ends_with(".bam") || path.ends_with(".sam") || path.ends_with(".cram") {
                Some(FileFormat::Bam)
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn is_stdin(&self) -> bool {
        self.input.is_empty()
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

    pub fn build_single_collection(&self) -> Result<fastx::Collection<BoxedReader>> {
        let collection = if !self.input.is_empty() {
            let mut readers = Vec::new();
            for path in &self.input {
                readers.push(load_reader(Some(path), self.batch_size)?);
            }
            fastx::Collection::new(readers, fastx::CollectionType::Single)
        } else {
            fastx::Collection::new(
                vec![self.build_single_reader()?],
                fastx::CollectionType::Single,
            )
        }?;
        Ok(collection)
    }

    pub fn build_paired_collection(&self) -> Result<fastx::Collection<BoxedReader>> {
        if self.input.is_empty() {
            bail!("Cannot build paired collection from stdin");
        }
        if !self.input.len().is_multiple_of(2) {
            bail!("Input must contain an even number of paths for paired collection");
        }
        let mut readers = Vec::new();
        for path in &self.input {
            readers.push(load_reader(Some(path), self.batch_size)?);
        }
        let collection = fastx::Collection::new(readers, fastx::CollectionType::Paired)?;
        Ok(collection)
    }

    pub fn build_interleaved_collection(&self) -> Result<fastx::Collection<BoxedReader>> {
        let collection = if !self.input.is_empty() {
            let mut readers = Vec::new();
            for path in &self.input {
                readers.push(load_reader(Some(path), self.batch_size)?);
            }
            fastx::Collection::new(readers, fastx::CollectionType::Interleaved)
        } else {
            fastx::Collection::new(
                vec![self.build_single_reader()?],
                fastx::CollectionType::Interleaved,
            )
        }?;
        Ok(collection)
    }
}

fn load_reader(
    path: Option<&str>,
    batch_size: Option<usize>,
) -> Result<fastx::Reader<BoxedReader>> {
    if let Some(path) = path {
        if path.starts_with("gs://") {
            #[cfg(not(feature = "gcs"))]
            {
                error!("Missing feature flag - gcs. To process Google Cloud Storage files, enable the 'gcs' feature flag.");
                bail!("Missing feature flag - gcs");
            }

            #[cfg(feature = "gcs")]
            Ok(load_gcs_reader(path, batch_size)?)
        } else {
            Ok(load_simple_reader(Some(path), batch_size)?)
        }
    } else {
        Ok(load_simple_reader(None, batch_size)?)
    }
}

fn load_simple_reader(
    path: Option<&str>,
    batch_size: Option<usize>,
) -> Result<fastx::Reader<BoxedReader>, paraseq::Error> {
    let path_display = if let Some(path) = path {
        path.to_string()
    } else {
        "stdin".to_string()
    };
    if let Some(size) = batch_size {
        debug!("building on-disk fastx reader with batch size {size} from: {path_display}");
        fastx::Reader::from_optional_path_with_batch_size(path, size)
    } else {
        debug!("building on-disk fastx reader from: {path_display}");
        fastx::Reader::from_optional_path(path)
    }
}

#[cfg(feature = "gcs")]
fn load_gcs_reader(
    path: &str,
    batch_size: Option<usize>,
) -> Result<fastx::Reader<BoxedReader>, paraseq::Error> {
    if let Some(size) = batch_size {
        debug!("building GCS fastx reader with batch size {size} from: {path}");
        fastx::Reader::from_gcs_with_batch_size(path, size)
    } else {
        debug!("building GCS fastx reader from: {path}");
        fastx::Reader::from_gcs(path)
    }
}

#[derive(Parser, Debug, Clone, PartialEq, Eq)]
#[clap(next_help_heading = "RECURSION OPTIONS")]
pub struct RecursiveOptions {
    /// Maximum depth in the directory tree to process. Leaving this option empty will set no limit.
    #[clap(long, requires = "recursive")]
    pub depth: Option<usize>,
}

#[derive(Parser, Debug, Clone, PartialEq, Eq)]
#[clap(next_help_heading = "BATCH ENCODING OPTIONS")]
pub struct BatchEncodingOptions {
    /// Encode *{_R1,_R2}* record pairs. Ignored unless `--manifest` or `--recursive` is specified.
    #[clap(short = 'P', long)]
    pub paired: bool,

    /// Collate all input files into a single output file. Will respect paired records if `--paired` is specified.
    #[clap(short = 'C', long)]
    pub collate: bool,
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
}

#[derive(Parser, Debug)]
#[clap(next_help_heading = "INPUT FILE OPTIONS")]
pub struct MultiInputBinseq {
    /// Input binseq files
    #[clap(num_args = 1..)]
    pub input: Vec<String>,
}
