use nix::errno::Errno;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use anyhow::Result;
use log::{trace, warn};
use nix::sys::stat;
use nix::unistd;

use super::{BoxedWriter, PairedChannels, RecordPair};
use crate::cli::FileFormat;

/// Creates many FIFOs (named-pipes) at the given basepath.
///
/// For paired files, `channels` controls which channels are created. For
/// unpaired files, `channels` is ignored and a single unlabelled FIFO per
/// thread is created.
///
/// Note: this does not open the FIFOs for writing.
pub fn create_fifos(
    basepath: &str,
    paired: bool,
    num_threads: usize,
    format: FileFormat,
    channels: PairedChannels,
) -> Result<Vec<String>> {
    let mut fifo_paths = Vec::new();
    if paired {
        for idx in 0..num_threads {
            if matches!(channels, PairedChannels::Both | PairedChannels::R1Only) {
                let path = name_fifo(basepath, idx, RecordPair::R1, format);
                create_fifo(&path)?;
                fifo_paths.push(path);
            }
            if matches!(channels, PairedChannels::Both | PairedChannels::R2Only) {
                let path = name_fifo(basepath, idx, RecordPair::R2, format);
                create_fifo(&path)?;
                fifo_paths.push(path);
            }
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
    trace!("Creating FIFO at path: {path}");
    match unistd::mkfifo(Path::new(path), stat::Mode::S_IRUSR | stat::Mode::S_IWUSR) {
        Ok(()) => Ok(()),
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
    trace!("Opened writer at FIFO path: {path}");
    Ok(Box::new(handle))
}

/// RAII guard that unlinks a set of FIFOs when dropped.
///
/// Cleanup is tied to the guard's lifetime rather than the happy path, so the
/// FIFOs are removed from disk on any early return, `?` propagation, or panic
/// (via stack unwinding) — not just on successful completion.
pub struct FifoGuard {
    paths: Vec<String>,
}

impl FifoGuard {
    pub fn new(paths: Vec<String>) -> Self {
        Self { paths }
    }

    /// The FIFO paths under guard.
    pub fn paths(&self) -> &[String] {
        &self.paths
    }
}

impl Drop for FifoGuard {
    fn drop(&mut self) {
        for path in &self.paths {
            close_fifo(path);
        }
    }
}

/// Close a FIFO (unlink the path).
///
/// A missing path (`ENOENT`) is treated as success so cleanup is idempotent;
/// other errors are logged but not propagated, since this runs during teardown.
fn close_fifo(path: &str) {
    trace!("Closing FIFO at path: {path}");
    match unistd::unlink(Path::new(path)) {
        Ok(()) | Err(Errno::ENOENT) => {}
        Err(err) => warn!("Failed to unlink FIFO at {path}: {err}"),
    }
}

pub fn name_fifo(basepath: &str, pid: usize, pair: RecordPair, format: FileFormat) -> String {
    match pair {
        RecordPair::R1 => format!("{}_{}_R1.{}", basepath, pid, format.extension()),
        RecordPair::R2 => format!("{}_{}_R2.{}", basepath, pid, format.extension()),
        RecordPair::Unpaired => format!("{}_{}.{}", basepath, pid, format.extension()),
    }
}
