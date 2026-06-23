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
            )
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
    pub fn extension(&self) -> Option<&'static str> {
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
