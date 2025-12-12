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
    let reader = BinseqReader::new(args.input.path())?;
    let num_pipes = args.num_pipes();
    let format = args.format()?;

    // Create all FIFOs first (don't open yet)
    let basename = args.basepath();
    let fifo_paths = create_fifos(basename, reader.is_paired(), num_pipes, format)?;
    info!(
        "{} FIFOs created. Waiting for readers to connect...",
        fifo_paths.len()
    );

    let num_records = reader.num_records()?;
    let records_per_pipe = num_records / num_pipes;

    let mut handles = Vec::new();
    for pid in 0..num_pipes {
        let paired = reader.is_paired();

        let rstart = records_per_pipe * pid;
        let rend = if pid == num_pipes - 1 {
            num_records
        } else {
            rstart + records_per_pipe
        };

        if paired {
            let pipe_basename = basename.to_string();
            let pipe_reader_path = args.input.path().to_string();
            let handle_r1 = thread::spawn(move || -> Result<()> {
                let handle_reader = BinseqReader::new(&pipe_reader_path)?;
                let proc = PipeProcessor::new(&pipe_basename, pid, format, true)?;
                handle_reader.process_parallel_range(proc, 1, rstart..rend)?;
                Ok(())
            });
            handles.push(handle_r1);

            let pipe_basename = basename.to_string();
            let pipe_reader_path = args.input.path().to_string();
            let handle_r2 = thread::spawn(move || -> Result<()> {
                let handle_reader = BinseqReader::new(&pipe_reader_path)?;
                let proc = PipeProcessor::new(&pipe_basename, pid, format, false)?;
                handle_reader.process_parallel_range(proc, 1, rstart..rend)?;
                Ok(())
            });
            handles.push(handle_r2);
        } else {
            let pipe_basename = basename.to_string();
            let pipe_reader_path = args.input.path().to_string();
            let handle = thread::spawn(move || -> Result<()> {
                let handle_reader = BinseqReader::new(&pipe_reader_path)?;
                let proc = PipeProcessor::new(&pipe_basename, pid, format, true)?;
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
