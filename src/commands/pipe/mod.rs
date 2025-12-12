pub mod processor;

use nix::errno::Errno;
use std::io::BufWriter;
use std::path::Path;
use std::thread;
use std::{fs::File, io::Write};

use anyhow::Result;
use binseq::BinseqReader;
use log::{info, trace};
use nix::sys::stat;
use nix::unistd;

use crate::cli::{FileFormat, PipeCommand};
use processor::PipeProcessor;

pub type BoxedWriter = Box<dyn Write + Send>;

pub fn run(args: &PipeCommand) -> Result<()> {
    let format = args.format()?;
    let reader = BinseqReader::new(args.input.path())?;
    let num_records = reader.num_records()?;
    let num_pipes = if reader.is_paired() {
        (args.num_pipes() / 2).max(1)
    } else {
        args.num_pipes()
    };

    // Create or connect all FIFOs first (don't open as writers yet)
    let basename = args.basepath();
    let fifo_paths = create_fifos(basename, reader.is_paired(), num_pipes, format)?;
    info!(
        "{} FIFOs created. Waiting for readers to connect...",
        fifo_paths.len()
    );

    // calculate the span for each pipe
    let records_per_pipe = num_records / num_pipes;

    // for each pipe we open a thread which handles the init and exit of the writer.
    //
    // this is necessary because named-pipes are blocking on init until a reader connects.
    // To open all pipes at once we need to wrap the writer in its own thread which initializes
    // and waits until a reader connects.
    let mut handles = Vec::new();
    for pid in 0..num_pipes {
        let is_paired = reader.is_paired();

        let rstart = records_per_pipe * pid;
        let rend = if pid == num_pipes - 1 {
            num_records
        } else {
            rstart + records_per_pipe
        };

        if is_paired {
            // Initialize a new thread for the first pipe
            let pipe_basename = basename.to_string();
            let pipe_reader_path = args.input.path().to_string();
            let handle_r1 = thread::spawn(move || -> Result<()> {
                let handle_reader = BinseqReader::new(&pipe_reader_path)?;
                let proc = PipeProcessor::new(&pipe_basename, pid, format, true, is_paired)?;
                handle_reader.process_parallel_range(proc, 1, rstart..rend)?;
                Ok(())
            });
            handles.push(handle_r1);

            // Initialize a new thread for the second pipe
            let pipe_basename = basename.to_string();
            let pipe_reader_path = args.input.path().to_string();
            let handle_r2 = thread::spawn(move || -> Result<()> {
                let handle_reader = BinseqReader::new(&pipe_reader_path)?;
                let proc = PipeProcessor::new(&pipe_basename, pid, format, false, is_paired)?;
                handle_reader.process_parallel_range(proc, 1, rstart..rend)?;
                Ok(())
            });
            handles.push(handle_r2);
        } else {
            // Initialize a new thread for the writer
            let pipe_basename = basename.to_string();
            let pipe_reader_path = args.input.path().to_string();
            let handle = thread::spawn(move || -> Result<()> {
                let handle_reader = BinseqReader::new(&pipe_reader_path)?;
                let proc = PipeProcessor::new(&pipe_basename, pid, format, true, is_paired)?;
                handle_reader.process_parallel_range(proc, 1, rstart..rend)?;
                Ok(())
            });
            handles.push(handle);
        }
    }

    // Wait for all threads to finish
    for handle in handles {
        handle.join().unwrap()?;
    }

    // close all FIFOs
    info!("Closing FIFOs");
    close_fifos(&fifo_paths)?;

    Ok(())
}

fn create_fifos(
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

fn create_fifo(path: &str) -> Result<()> {
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

fn open_fifo(path: &str) -> Result<BoxedWriter> {
    let handle = File::options().write(true).open(path).map(BufWriter::new)?;
    trace!("Opened writer at FIFO path: {}", path);
    Ok(Box::new(handle))
}

fn close_fifos(paths: &[String]) -> Result<()> {
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
