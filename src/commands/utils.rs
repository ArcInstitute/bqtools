use std::{
    fs::{self, File},
    io::{self, BufWriter, Write},
    path::Path,
};

use anyhow::{bail, Result};
use gzp::{
    deflate::Gzip,
    par::compress::{ParCompress, ParCompressBuilder},
};
use log::trace;

pub fn make_directory<P: AsRef<Path>>(path: P) -> Result<()> {
    if path.as_ref().exists() {
        if path.as_ref().is_dir() {
            trace!(
                "Skipping directory creation for existing directory: {}",
                path.as_ref().display()
            );
        } else {
            bail!(
                "Cannot create directory at existing file path: {}",
                path.as_ref().display()
            );
        }
    } else {
        trace!("creating directory: {}", path.as_ref().display());
        fs::create_dir_all(path)?;
    }
    Ok(())
}

pub fn match_output<P: AsRef<Path>>(path: Option<P>) -> Result<Box<dyn Write + Send>> {
    if let Some(path) = path {
        trace!("Opening writer handle at: {}", path.as_ref().display());
        let handle = File::create(path)?;
        let buffer = BufWriter::new(handle);
        let boxed = Box::new(buffer);
        Ok(boxed)
    } else {
        trace!("Opening writer handle to stdout");
        let handle = io::stdout();
        let buffer = BufWriter::new(handle);
        let boxed = Box::new(buffer);
        Ok(boxed)
    }
}

#[derive(Clone, Copy, Default, Debug, clap::ValueEnum)]
pub enum CompressionType {
    #[default]
    #[value(name = "u")]
    Uncompressed,
    #[value(name = "g")]
    Gzip,
    #[value(name = "z")]
    Zstd,
}
impl CompressionType {
    pub fn extension(self) -> Option<&'static str> {
        match self {
            CompressionType::Uncompressed => None,
            CompressionType::Gzip => Some("gz"),
            CompressionType::Zstd => Some("zst"),
        }
    }
}

pub fn compress_passthrough(
    writer: Box<dyn Write + Send>,
    compression_type: CompressionType,
    num_threads: usize,
) -> Result<Box<dyn Write + Send>> {
    match compression_type {
        CompressionType::Uncompressed => Ok(writer),
        CompressionType::Gzip => compress_gzip_passthrough(writer, num_threads),
        CompressionType::Zstd => compress_zstd_passthrough(writer, 3, num_threads),
    }
}

pub fn compress_gzip_passthrough(
    writer: Box<dyn Write + Send>,
    num_threads: usize,
) -> Result<Box<dyn Write + Send>> {
    let encoder: ParCompress<Gzip, _> = ParCompressBuilder::new()
        .num_threads(num_threads)?
        .from_writer(writer);
    Ok(Box::new(encoder))
}

pub fn compress_zstd_passthrough(
    writer: Box<dyn Write + Send>,
    level: i32,
    num_threads: usize,
) -> Result<Box<dyn Write + Send>> {
    let mut encoder = zstd::Encoder::new(writer, level)?;
    encoder.multithread(num_threads as u32)?;
    let encoder = encoder.auto_finish();
    Ok(Box::new(encoder))
}

/// Default `max_n_frac` for fuzzy (sassy) matching: `k / pattern_length`.
///
/// Mirrors sassy's semantics for the fraction of `N` bases tolerated within a
/// match. Falls back to `1.0` (no restriction) when there is no pattern to
/// measure a length from.
#[cfg(feature = "fuzzy")]
pub fn default_max_n_frac(k: usize, pattern_len: usize) -> f32 {
    if pattern_len == 0 {
        1.0
    } else {
        (k as f32 / pattern_len as f32).min(1.0)
    }
}

/// Validates that every pattern in a fuzzy pattern set has the same length.
///
/// `sassy::Searcher::encode_patterns` requires uniform pattern lengths within a
/// single batch and otherwise panics via an internal `assert!`. Calling this
/// first turns that panic into a catchable error before patterns ever reach sassy.
#[cfg(feature = "fuzzy")]
pub fn validate_uniform_pattern_length(patterns: &[Vec<u8>]) -> Result<()> {
    let Some(expected) = patterns.first().map(Vec::len) else {
        return Ok(());
    };
    if let Some(bad) = patterns.iter().find(|p| p.len() != expected) {
        log::error!("Multiple pattern lengths provided - currently cannot handle variable-length patterns in fuzzy matching");
        bail!(
            "Pattern length mismatch: expected length {expected}, found length {}",
            bad.len()
        );
    }
    Ok(())
}
