use std::{
    fs::File,
    io::{self, BufReader, BufWriter, Read, Write},
};

use anyhow::Result;

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

pub fn match_output(path: Option<&String>) -> Result<Box<dyn Write>> {
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
