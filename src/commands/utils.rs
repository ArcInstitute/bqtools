use std::{
    fs::File,
    io::{self, BufReader, BufWriter, Read, Write},
};

use anyhow::Result;
use gzp::{
    deflate::Gzip,
    par::compress::{ParCompress, ParCompressBuilder},
};

pub fn match_input(path: Option<&String>) -> Result<Box<dyn Read>> {
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

pub fn compress_output_passthrough(
    writer: Box<dyn Write + Send>,
    compress: bool,
    num_threads: usize,
) -> Result<Box<dyn Write>> {
    if compress {
        let encoder: ParCompress<Gzip> = ParCompressBuilder::new()
            .num_threads(num_threads)?
            .from_writer(writer);
        Ok(Box::new(encoder))
    } else {
        Ok(writer)
    }
}
