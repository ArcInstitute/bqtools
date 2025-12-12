use nix::errno::Errno;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use anyhow::Result;
use log::trace;
use nix::sys::stat;
use nix::unistd;

use super::BoxedWriter;
use crate::cli::FileFormat;

pub fn create_fifos(
    basepath: &str,
    paired: bool,
    num_threads: usize,
    format: FileFormat,
) -> Result<Vec<String>> {
    let mut fifo_paths = Vec::new();
    if paired {
        for idx in 0..num_threads {
            let path_r1 = format!("{}_{}_R1.{}", basepath, idx, format.extension());
            let path_r2 = format!("{}_{}_R2.{}", basepath, idx, format.extension());
            create_fifo(&path_r1)?;
            create_fifo(&path_r2)?;
            fifo_paths.push(path_r1);
            fifo_paths.push(path_r2);
        }
    } else {
        for idx in 0..num_threads {
            let path = format!("{}_{}.{}", basepath, idx, format.extension());
            create_fifo(&path)?;
            fifo_paths.push(path);
        }
    }
    Ok(fifo_paths)
}

pub fn create_fifo(path: &str) -> Result<()> {
    trace!("Creating FIFO at path: {}", path);
    match unistd::mkfifo(Path::new(path), stat::Mode::S_IRUSR | stat::Mode::S_IWUSR) {
        Ok(_) => Ok(()),
        Err(Errno::EEXIST) => {
            trace!("FIFO already exists at {path}, reconnecting...");
            Ok(())
        }
        Err(err) => Err(err.into()),
    }
}

pub fn open_fifo(path: &str) -> Result<BoxedWriter> {
    let handle = File::options().write(true).open(path).map(BufWriter::new)?;
    trace!("Opened writer at FIFO path: {}", path);
    Ok(Box::new(handle))
}

pub fn close_fifos(paths: &[String]) -> Result<()> {
    let mut result = Ok(());
    for path in paths {
        if let Err(err) = close_fifo(path) {
            result = Err(err);
        }
    }
    result
}

fn close_fifo(path: &str) -> Result<()> {
    trace!("Closing FIFO at path: {}", path);
    unistd::unlink(Path::new(path))?;
    Ok(())
}
