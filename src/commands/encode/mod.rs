use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use binseq::{bq::BinseqHeader, vbq::VBinseqHeader, Policy};
use log::{debug, error, info, trace};
use paraseq::{
    fastx::{self, Format},
    htslib,
    prelude::*,
};
use regex::Regex;
use walkdir::WalkDir;

use crate::{
    cli::{BinseqMode, EncodeCommand, FileFormat},
    commands::{
        encode::utils::{
            generate_output_name, get_sequence_len_htslib, pair_r1_r2_files, pull_single_files,
        },
        utils::match_output,
    },
    types::BoxedReader,
};

mod processor;
mod utils;

use processor::{BinseqProcessor, VBinseqProcessor};
use utils::{get_interleaved_sequence_len, get_sequence_len};

fn encode_single(
    mut reader: fastx::Reader<BoxedReader>,
    out_path: Option<&str>,
    mode: BinseqMode,
    num_threads: usize,
    compress: bool,
    quality: bool,
    block_size: usize,
    policy: Policy,
) -> Result<(usize, usize)> {
    // build writer
    let out_handle = match_output(out_path)?;

    let (num_records, num_skipped) = if mode == BinseqMode::Binseq {
        trace!("converting to bq");

        // Determine the sequence length
        let slen = get_sequence_len(&mut reader)?;
        trace!("sequence length: {}", slen);

        let header = BinseqHeader::new(slen);
        let processor = BinseqProcessor::new(header, policy, out_handle)?;

        // Process the records in parallel
        trace!("processing records in parallel (T={})", num_threads);
        reader.process_parallel(processor.clone(), num_threads)?;

        // Update the number of records
        let num_records = processor.get_global_record_count();
        let num_skipped = processor.get_global_skipped_count();

        (num_records, num_skipped)
    } else {
        trace!("converting to vbq");
        let quality = match reader.format() {
            Format::Fastq => quality,
            Format::Fasta => false, // never record fasta quality
        };
        trace!("quality: {}", quality);
        let header = VBinseqHeader::with_capacity(block_size as u64, quality, compress, false);
        let processor = VBinseqProcessor::new(header, policy, out_handle)?;

        // Process the records in parallel
        trace!("processing records in parallel (T={})", num_threads);
        reader.process_parallel(processor.clone(), num_threads)?;
        processor.finish()?;

        // Update the number of records
        let num_records = processor.get_global_record_count();
        let num_skipped = processor.get_global_skipped_count();

        (num_records, num_skipped)
    };

    Ok((num_records, num_skipped))
}

fn encode_single_htslib(
    in_path: &str,
    out_path: Option<&str>,
    mode: BinseqMode,
    num_threads: usize,
    compress: bool,
    quality: bool,
    block_size: usize,
    policy: Policy,
) -> Result<(usize, usize)> {
    // build reader
    let reader = htslib::Reader::from_path(in_path)?;

    // build writer
    let out_handle = match_output(out_path)?;

    let (num_records, num_skipped) = if mode == BinseqMode::Binseq {
        trace!("converting to bq");

        // Determine the sequence length
        let (slen, _) = get_sequence_len_htslib(in_path, false)?;
        trace!("sequence length: {}", slen);

        let header = BinseqHeader::new(slen);
        let processor = BinseqProcessor::new(header, policy, out_handle)?;

        // Process the records in parallel
        trace!("processing records in parallel (T={})", num_threads);
        reader.process_parallel(processor.clone(), num_threads)?;

        // Update the number of records
        let num_records = processor.get_global_record_count();
        let num_skipped = processor.get_global_skipped_count();

        (num_records, num_skipped)
    } else {
        trace!("converting to vbq");
        let header = VBinseqHeader::with_capacity(block_size as u64, quality, compress, false);
        let processor = VBinseqProcessor::new(header, policy, out_handle)?;

        // Process the records in parallel
        trace!("processing records in parallel (T={})", num_threads);
        reader.process_parallel(processor.clone(), num_threads)?;
        processor.finish()?;

        // Update the number of records
        let num_records = processor.get_global_record_count();
        let num_skipped = processor.get_global_skipped_count();

        (num_records, num_skipped)
    };

    Ok((num_records, num_skipped))
}

fn encode_interleaved(
    mut reader: fastx::Reader<BoxedReader>,
    out_path: Option<&str>,
    mode: BinseqMode,
    num_threads: usize,
    compress: bool,
    quality: bool,
    block_size: usize,
    policy: Policy,
) -> Result<(usize, usize)> {
    // Prepare the processor
    let out_handle = match_output(out_path)?;

    let (num_records, num_skipped) = if mode == BinseqMode::Binseq {
        trace!("converting to bq");

        // Determine the sequence length
        let (slen, xlen) = get_interleaved_sequence_len(&mut reader)?;
        trace!("sequence length: slen={}, xlen={}", slen, xlen);

        let header = BinseqHeader::new_extended(slen, xlen);
        let processor = BinseqProcessor::new(header, policy, out_handle)?;

        // Process the records in parallel
        trace!("processing records in parallel (T={})", num_threads);
        reader.process_parallel_interleaved(processor.clone(), num_threads)?;

        // Update the number of records
        let num_records = processor.get_global_record_count();
        let num_skipped = processor.get_global_skipped_count();

        (num_records, num_skipped)
    } else {
        trace!("converting to vbq");
        let quality = match reader.format() {
            Format::Fastq => quality,
            Format::Fasta => false, // never record quality for fasta
        };
        trace!("quality: {}", quality);
        let header = VBinseqHeader::with_capacity(block_size as u64, quality, compress, true);
        let processor = VBinseqProcessor::new(header, policy, out_handle)?;

        // Process the records in parallel
        trace!("processing records in parallel (T={})", num_threads);
        reader.process_parallel_interleaved(processor.clone(), num_threads)?;
        processor.finish()?;

        // Update the number of records
        let num_records = processor.get_global_record_count();
        let num_skipped = processor.get_global_skipped_count();

        (num_records, num_skipped)
    };

    Ok((num_records, num_skipped))
}

fn encode_interleaved_htslib(
    in_path: &str,
    out_path: Option<&str>,
    mode: BinseqMode,
    num_threads: usize,
    compress: bool,
    quality: bool,
    block_size: usize,
    _batch_size: Option<usize>,
    policy: Policy,
) -> Result<(usize, usize)> {
    let reader = htslib::Reader::from_path(in_path)?;

    // Prepare the processor
    let out_handle = match_output(out_path)?;

    let (num_records, num_skipped) = if mode == BinseqMode::Binseq {
        trace!("converting to bq");

        // Determine the sequence length
        let (slen, xlen) = get_sequence_len_htslib(in_path, true)?;
        trace!("sequence length: {}, xlen: {}", slen, xlen);

        let header = BinseqHeader::new_extended(slen, xlen);
        let processor = BinseqProcessor::new(header, policy, out_handle)?;

        // Process the records in parallel
        trace!("processing records in parallel (T={})", num_threads);
        reader.process_parallel_interleaved(processor.clone(), num_threads)?;

        // Update the number of records
        let num_records = processor.get_global_record_count();
        let num_skipped = processor.get_global_skipped_count();

        (num_records, num_skipped)
    } else {
        trace!("converting to vbq");
        let header = VBinseqHeader::with_capacity(block_size as u64, quality, compress, true);
        let processor = VBinseqProcessor::new(header, policy, out_handle)?;

        // Process the records in parallel
        trace!("processing records in parallel (T={})", num_threads);
        reader.process_parallel_interleaved(processor.clone(), num_threads)?;
        processor.finish()?;

        // Update the number of records
        let num_records = processor.get_global_record_count();
        let num_skipped = processor.get_global_skipped_count();

        (num_records, num_skipped)
    };

    Ok((num_records, num_skipped))
}

fn encode_paired(
    mut reader_r1: fastx::Reader<BoxedReader>,
    mut reader_r2: fastx::Reader<BoxedReader>,
    out_path: Option<&str>,
    mode: BinseqMode,
    num_threads: usize,
    compress: bool,
    quality: bool,
    block_size: usize,
    policy: Policy,
) -> Result<(usize, usize)> {
    // Prepare the output handle
    let out_handle = match_output(out_path)?;

    let (num_records, num_skipped) = match mode {
        BinseqMode::Binseq => {
            trace!("converting to bq");

            // Determine the sequence length
            let slen = get_sequence_len(&mut reader_r1)?;
            let xlen = get_sequence_len(&mut reader_r2)?;
            trace!("sequence length: slen={}, xlen={}", slen, xlen);

            // Prepare the output HEADER
            let header = BinseqHeader::new_extended(slen, xlen);
            let processor = BinseqProcessor::new(header, policy, out_handle)?;

            // Process the records in parallel
            trace!("processing records in parallel (T={})", num_threads);
            reader_r1.process_parallel_paired(reader_r2, processor.clone(), num_threads)?;

            // Update the number of records
            let num_records = processor.get_global_record_count();
            let num_skipped = processor.get_global_skipped_count();

            (num_records, num_skipped)
        }
        BinseqMode::VBinseq => {
            trace!("converting to vbq");

            let quality = match reader_r1.format() {
                Format::Fastq => quality,
                Format::Fasta => false, // never record quality for fasta
            };
            trace!("quality: {}", quality);

            let header = VBinseqHeader::with_capacity(block_size as u64, quality, compress, true);
            let processor = VBinseqProcessor::new(header, policy, out_handle)?;

            // Process the records in parallel
            trace!("processing records in parallel (T={})", num_threads);
            reader_r1.process_parallel_paired(reader_r2, processor.clone(), num_threads)?;
            processor.finish()?;

            // Update the number of records
            let num_records = processor.get_global_record_count();
            let num_skipped = processor.get_global_skipped_count();

            (num_records, num_skipped)
        }
    };

    Ok((num_records, num_skipped))
}

/// Run the encoding process for an atomic single/paired input
fn run_atomic(args: &EncodeCommand) -> Result<()> {
    let (num_records, num_skipped) = if args.input.paired() {
        trace!("launching paired encoding");
        let (rdr1, rdr2) = args.input.build_paired_readers()?;
        encode_paired(
            rdr1,
            rdr2,
            args.output.borrowed_path(),
            args.output.mode()?,
            args.output.threads(),
            args.output.compress(),
            args.output.quality(),
            args.output.block_size,
            args.output.policy.into(),
        )
    } else if args.input.interleaved {
        if let Some(FileFormat::Bam) = args.input.format {
            trace!("launching interleaved encoding (htslib)");
            encode_interleaved_htslib(
                args.input
                    .single_path()?
                    .context("Must provide an input path for HTSLib")?,
                args.output.borrowed_path(),
                args.output.mode()?,
                args.output.threads(),
                args.output.compress(),
                args.output.quality(),
                args.output.block_size,
                args.input.batch_size,
                args.output.policy.into(),
            )
        } else {
            trace!("launching interleaved encoding (fastx)");
            encode_interleaved(
                args.input.build_single_reader()?,
                args.output.borrowed_path(),
                args.output.mode()?,
                args.output.threads(),
                args.output.compress(),
                args.output.quality(),
                args.output.block_size,
                args.output.policy.into(),
            )
        }
    } else if let Some(FileFormat::Bam) = args.input.format {
        trace!("launching single encoding (htslib)");
        encode_single_htslib(
            args.input
                .single_path()?
                .context("Must provide an input path for HTSlib")?,
            args.output.borrowed_path(),
            args.output.mode()?,
            args.output.threads(),
            args.output.compress(),
            args.output.quality(),
            args.output.block_size,
            args.output.policy.into(),
        )
    } else {
        trace!("launching single encoding (fastx)");
        encode_single(
            args.input.build_single_reader()?,
            args.output.borrowed_path(),
            args.output.mode()?,
            args.output.threads(),
            args.output.compress(),
            args.output.quality(),
            args.output.block_size,
            args.output.policy.into(),
        )
    }?;

    if let Some(opath) = args.output.borrowed_path() {
        info!("Wrote {num_records} records to: {opath}");
    } else {
        info!("Wrote {num_records} records to: stdout");
    }
    if num_skipped > 0 {
        info!("Skipped {num_skipped} records");
    }

    if args.output.index
        && args.output.mode()? == BinseqMode::VBinseq
        && args.output.output.is_some()
    {
        crate::commands::index::index_path(args.output.borrowed_path().unwrap(), true)?;
    }

    Ok(())
}

fn process_queue(args: &EncodeCommand, queue: Vec<Vec<PathBuf>>, regex: &Regex) -> Result<()> {
    let num_threads = args.output.threads();

    // Case where there are more threads than files
    if queue.len() <= num_threads {
        let base_threads_per_file = num_threads / queue.len();
        let leftover_threads = num_threads % queue.len();

        info!(
            "Distributing {} threads across {} files",
            num_threads,
            queue.len()
        );
        if leftover_threads > 0 {
            debug!(
                "Base threads per file: {base_threads_per_file}, extra threads for first {leftover_threads} file(s)"
            );
        } else {
            debug!("Threads per file: {base_threads_per_file}");
        }

        let mut handles = vec![];
        for (i, pair) in queue.into_iter().enumerate() {
            let thread_args = args.clone();
            let thread_regex = regex.clone();
            let mode = args.output.mode()?;

            // First `leftover_threads` files get one extra thread
            let threads_for_this_file = if i < leftover_threads {
                base_threads_per_file + 1
            } else {
                base_threads_per_file
            };

            let handle = std::thread::spawn(move || -> Result<()> {
                let mut file_args = thread_args.clone();

                match pair.len() {
                    1 => {
                        let inpath = pair[0].to_str().unwrap().to_string();
                        let outpath = thread_regex
                            .replace_all(&inpath, mode.extension())
                            .to_string();
                        file_args.input.input = vec![inpath];
                        file_args.output.output = Some(outpath);
                        file_args.output.threads = threads_for_this_file;
                    }
                    2 => {
                        let inpaths: Vec<String> = pair
                            .iter()
                            .map(|path| path.to_str().unwrap().to_string())
                            .collect();
                        let outpath = generate_output_name(&pair, mode.extension())?;

                        file_args.input.input = inpaths;
                        file_args.output.output = Some(outpath);
                        file_args.output.threads = threads_for_this_file;
                    }
                    _ => {
                        bail!("Invalid number of input files found: {}", pair.len())
                    }
                }

                run_atomic(&file_args)?;
                Ok(())
            });
            handles.push(handle);
        }

        for handle in handles {
            if let Err(err) = handle.join() {
                error!("Error in thread: {err:?}");
            }
        }

    // Case where there are more files than threads (batching)
    } else {
        let mut num_processed = 0;
        loop {
            let rbound = (num_processed + num_threads).min(queue.len());
            if num_processed == rbound {
                break;
            }
            let subqueue = queue[num_processed..rbound].to_vec();
            num_processed += subqueue.len();
            process_queue(args, subqueue, regex)?;
        }
    }

    Ok(())
}

fn run_recursive(args: &EncodeCommand) -> Result<()> {
    let args = args.to_owned();
    let dir = args.input.as_directory()?;

    let regex_str = if args.input.recursion.paired {
        r"_R[12](_[^.]*)?\.(?:fastq|fq|fasta|fa)(?:\.gz|\.zst)?$"
    } else {
        r"\.(fastq|fq|fasta|fa)(\.gz|\.zst)?$"
    };

    let regex = Regex::new(regex_str)?;

    let mut fqueue = vec![];
    info!("Processing files in directory: {}", dir.display());

    let dir_walker = if let Some(max_depth) = args.input.recursion.depth {
        WalkDir::new(dir).max_depth(max_depth)
    } else {
        WalkDir::new(dir)
    };
    for entry in dir_walker {
        let entry = entry?;
        let path = entry.path();
        let path_str = path.as_os_str().to_str().unwrap();
        if path.is_file() && regex.is_match(path_str) {
            fqueue.push(path.to_owned());
        }
    }
    if fqueue.is_empty() {
        bail!("No files found");
    }
    fqueue.sort_unstable();

    let pqueue = if args.input.recursion.paired {
        pair_r1_r2_files(&fqueue)
    } else {
        pull_single_files(&fqueue)
    }?;

    if pqueue.is_empty() {
        bail!("No files found matching the expected pattern.");
    }

    if args.input.recursion.paired {
        info!("Total file pairs found: {}", pqueue.len());
    } else {
        info!("Total files found: {}", pqueue.len());
    }

    process_queue(&args, pqueue, &regex)?;

    Ok(())
}

pub fn run(args: &EncodeCommand) -> Result<()> {
    if args.input.recursive {
        trace!("launching encode-recursive");
        run_recursive(args)
    } else {
        trace!("launching encode-atomic");
        run_atomic(args)
    }
}
