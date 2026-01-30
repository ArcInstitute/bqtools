pub mod processor;
pub mod utils;

use std::io::Write;
use std::thread;

use anyhow::Result;
use binseq::BinseqReader;
use log::{info, warn};

use crate::cli::{FileFormat, PipeCommand};
use processor::PipeProcessor;
use utils::{close_fifos, create_fifos};

pub type BoxedWriter = Box<dyn Write + Send>;

/// Simple enum to represent the type of record pair to process.
#[derive(Clone, Copy, Debug)]
pub enum RecordPair {
    R1,
    R2,
    Unpaired,
}

pub fn run(args: &PipeCommand) -> Result<()> {
    if args.input.span.is_some() {
        warn!("Span is ignored when using pipe subcommand");
    }

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
        let rstart = records_per_pipe * pid;
        let rend = if pid == num_pipes - 1 {
            num_records
        } else {
            rstart + records_per_pipe
        };

        if reader.is_paired() {
            handles.push(spawn_pipe_thread(
                basename.to_string(),
                args.input.path().to_string(),
                pid,
                format,
                RecordPair::R1,
                rstart..rend,
            ));
            handles.push(spawn_pipe_thread(
                basename.to_string(),
                args.input.path().to_string(),
                pid,
                format,
                RecordPair::R2,
                rstart..rend,
            ));
        } else {
            handles.push(spawn_pipe_thread(
                basename.to_string(),
                args.input.path().to_string(),
                pid,
                format,
                RecordPair::Unpaired,
                rstart..rend,
            ));
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

fn spawn_pipe_thread(
    basename: String,
    input_path: String,
    pid: usize,
    format: FileFormat,
    record_pair: RecordPair,
    range: std::ops::Range<usize>,
) -> thread::JoinHandle<Result<()>> {
    thread::spawn(move || -> Result<()> {
        let handle_reader = BinseqReader::new(&input_path)?;
        let proc = PipeProcessor::new(&basename, pid, format, record_pair)?;
        handle_reader.process_parallel_range(proc, 1, range)?;
        Ok(())
    })
}
