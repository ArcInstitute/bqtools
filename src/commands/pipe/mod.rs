pub mod processor;
pub mod utils;

use std::io::Write;
use std::thread;

use anyhow::Result;
use binseq::BinseqReader;
use log::info;

use crate::cli::PipeCommand;
use processor::PipeProcessor;
use utils::{close_fifos, create_fifos};

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
