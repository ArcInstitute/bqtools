use nix::errno::Errno;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use anyhow::Result;
use log::trace;
use nix::sys::stat;
use nix::unistd;

use super::{BoxedWriter, RecordPair};
use crate::cli::FileFormat;

/// Creates many FIFOs (named-pipes) at the given basepath
///
/// Note: this does not open the FIFOs for writing
pub fn create_fifos(
    basepath: &str,
    paired: bool,
    num_threads: usize,
    format: FileFormat,
) -> Result<Vec<String>> {
    let mut fifo_paths = Vec::new();
    if paired {
        for idx in 0..num_threads {
            let path_r1 = name_fifo(basepath, idx, RecordPair::R1, format);
            let path_r2 = name_fifo(basepath, idx, RecordPair::R2, format);
            create_fifo(&path_r1)?;
            create_fifo(&path_r2)?;
            fifo_paths.push(path_r1);
            fifo_paths.push(path_r2);
        }
    } else {
        for idx in 0..num_threads {
            let path = name_fifo(basepath, idx, RecordPair::Unpaired, format);
            create_fifo(&path)?;
            fifo_paths.push(path);
        }
    }
    Ok(fifo_paths)
}

/// Create a FIFO (named-pipe) at the given path
///
/// Note: this does not open the FIFO for writing
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

/// Open a FIFO for writing
pub fn open_fifo(path: &str) -> Result<BoxedWriter> {
    let handle = File::options().write(true).open(path).map(BufWriter::new)?;
    trace!("Opened writer at FIFO path: {}", path);
    Ok(Box::new(handle))
}

/// Close many FIFOs (unlink the path)
pub fn close_fifos(paths: &[String]) -> Result<()> {
    let mut result = Ok(());
    for path in paths {
        if let Err(err) = close_fifo(path) {
            result = Err(err);
        }
    }
    result
}

/// Close a FIFO (unlink the path)
pub fn close_fifo(path: &str) -> Result<()> {
    trace!("Closing FIFO at path: {}", path);
    unistd::unlink(Path::new(path))?;
    Ok(())
}

pub fn name_fifo(basepath: &str, pid: usize, pair: RecordPair, format: FileFormat) -> String {
    match pair {
        RecordPair::R1 => format!("{}_{}_R1.{}", basepath, pid, format.extension()),
        RecordPair::R2 => format!("{}_{}_R2.{}", basepath, pid, format.extension()),
        RecordPair::Unpaired => format!("{}_{}.{}", basepath, pid, format.extension()),
    }
}
