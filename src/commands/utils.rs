use std::{
    fs::{File, OpenOptions},
    io::{self, BufReader, BufWriter, Read, Write},
};

use anyhow::Result;
use gzp::{
    deflate::Gzip,
    par::compress::{ParCompress, ParCompressBuilder},
};

pub fn match_input(path: Option<&String>) -> Result<Box<dyn Read + Send>> {
    if let Some(path) = path {
        let handle = File::open(path)?;
        let buffer = BufReader::new(handle);
        let boxed = Box::new(buffer);
        Ok(boxed)
    } else {
        let handle = io::stdin();
        let buffer = BufReader::new(handle);
        let boxed = Box::new(buffer);
        Ok(boxed)
    }
}

pub fn match_output(path: Option<&String>) -> Result<Box<dyn Write + Send>> {
    if let Some(path) = path {
        let handle = File::create(path)?;
        let buffer = BufWriter::new(handle);
        let boxed = Box::new(buffer);
        Ok(boxed)
    } else {
        let handle = io::stdout();
        let buffer = BufWriter::new(handle);
        let boxed = Box::new(buffer);
        Ok(boxed)
    }
}

pub fn reopen_output(path: Option<&String>) -> Result<Box<dyn Write + Send>> {
    if let Some(path) = path {
        let handle = OpenOptions::new()
            .create_new(false)
            .append(true)
            .open(path)?;
        let buffer = BufWriter::new(handle);
        let boxed = Box::new(buffer);
        Ok(boxed)
    } else {
        let handle = io::stdout();
        let buffer = BufWriter::new(handle);
        let boxed = Box::new(buffer);
        Ok(boxed)
    }
}

pub fn compress_gzip_passthrough(
    writer: Box<dyn Write + Send>,
    compress: bool,
    num_threads: usize,
) -> Result<Box<dyn Write + Send>> {
    if compress {
        let encoder: ParCompress<Gzip> = ParCompressBuilder::new()
            .num_threads(num_threads)?
            .from_writer(writer);
        Ok(Box::new(encoder))
    } else {
        Ok(writer)
    }
}

pub fn compress_zstd_passthrough(
    writer: Box<dyn Write + Send>,
    compress: bool,
    level: i32,
    num_threads: usize,
) -> Result<Box<dyn Write + Send>> {
    if compress {
        let mut encoder = zstd::Encoder::new(writer, level)?;
        encoder.multithread(num_threads as u32)?;
        let encoder = encoder.auto_finish();
        Ok(Box::new(encoder))
    } else {
        Ok(writer)
    }
}
