use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use binseq::{bq::BinseqHeader, vbq::VBinseqHeader, Policy};
use paraseq::{
    fastx::{self, Format, Reader},
    htslib,
    prelude::*,
};
use regex::Regex;
use walkdir::WalkDir;

use crate::{
    cli::{BinseqMode, EncodeCommand, FileFormat},
    commands::{
        encode::utils::{generate_output_name, get_sequence_len_htslib, pair_r1_r2_files},
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
        // Determine the sequence length
        let slen = get_sequence_len(&mut reader)?;

        let header = BinseqHeader::new(slen);
        let processor = BinseqProcessor::new(header, policy, out_handle)?;

        // Process the records in parallel
        reader.process_parallel(processor.clone(), num_threads)?;

        // Update the number of records
        let num_records = processor.get_global_record_count();
        let num_skipped = processor.get_global_skipped_count();

        (num_records, num_skipped)
    } else {
        let quality = match reader.format() {
            Format::Fastq => quality,
            Format::Fasta => false, // never record fasta quality
        };
        let header = VBinseqHeader::with_capacity(block_size as u64, quality, compress, false);
        let processor = VBinseqProcessor::new(header, policy, out_handle)?;

        // Process the records in parallel
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
        // Determine the sequence length
        let (slen, _) = get_sequence_len_htslib(in_path, false)?;

        let header = BinseqHeader::new(slen);
        let processor = BinseqProcessor::new(header, policy, out_handle)?;

        // Process the records in parallel
        reader.process_parallel(processor.clone(), num_threads)?;

        // Update the number of records
        let num_records = processor.get_global_record_count();
        let num_skipped = processor.get_global_skipped_count();

        (num_records, num_skipped)
    } else {
        let header = VBinseqHeader::with_capacity(block_size as u64, quality, compress, false);
        let processor = VBinseqProcessor::new(header, policy, out_handle)?;

        // Process the records in parallel
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
        // Determine the sequence length
        let (slen, xlen) = get_interleaved_sequence_len(&mut reader)?;

        let header = BinseqHeader::new_extended(slen, xlen);
        let processor = BinseqProcessor::new(header, policy, out_handle)?;

        // Process the records in parallel
        reader.process_parallel_interleaved(processor.clone(), num_threads)?;

        // Update the number of records
        let num_records = processor.get_global_record_count();
        let num_skipped = processor.get_global_skipped_count();

        (num_records, num_skipped)
    } else {
        let quality = match reader.format() {
            Format::Fastq => quality,
            Format::Fasta => false, // never record quality for fasta
        };
        let header = VBinseqHeader::with_capacity(block_size as u64, quality, compress, true);
        let processor = VBinseqProcessor::new(header, policy, out_handle)?;

        // Process the records in parallel
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
        // Determine the sequence length
        let (slen, xlen) = get_sequence_len_htslib(in_path, true)?;

        let header = BinseqHeader::new_extended(slen, xlen);
        let processor = BinseqProcessor::new(header, policy, out_handle)?;

        // Process the records in parallel
        reader.process_parallel_interleaved(processor.clone(), num_threads)?;

        // Update the number of records
        let num_records = processor.get_global_record_count();
        let num_skipped = processor.get_global_skipped_count();

        (num_records, num_skipped)
    } else {
        let header = VBinseqHeader::with_capacity(block_size as u64, quality, compress, true);
        let processor = VBinseqProcessor::new(header, policy, out_handle)?;

        // Process the records in parallel
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
    in_path1: &str,
    in_path2: &str,
    out_path: Option<&str>,
    mode: BinseqMode,
    num_threads: usize,
    compress: bool,
    quality: bool,
    block_size: usize,
    batch_size: Option<usize>,
    policy: Policy,
) -> Result<(usize, usize)> {
    let (mut reader_r1, mut reader_r2) = if let Some(size) = batch_size {
        (
            Reader::from_path_with_batch_size(in_path1, size)?,
            Reader::from_path_with_batch_size(in_path2, size)?,
        )
    } else {
        (Reader::from_path(in_path1)?, Reader::from_path(in_path2)?)
    };

    // Prepare the output handle
    let out_handle = match_output(out_path)?;

    let (num_records, num_skipped) = match mode {
        BinseqMode::Binseq => {
            // Determine the sequence length
            let slen = get_sequence_len(&mut reader_r1)?;
            let xlen = get_sequence_len(&mut reader_r2)?;

            // Prepare the output HEADER
            let header = BinseqHeader::new_extended(slen, xlen);
            let processor = BinseqProcessor::new(header, policy, out_handle)?;

            // Process the records in parallel
            reader_r1.process_parallel_paired(reader_r2, processor.clone(), num_threads)?;

            // Update the number of records
            let num_records = processor.get_global_record_count();
            let num_skipped = processor.get_global_skipped_count();

            (num_records, num_skipped)
        }
        BinseqMode::VBinseq => {
            let quality = match reader_r1.format() {
                Format::Fastq => quality,
                Format::Fasta => false, // never record quality for fasta
            };
            let header = VBinseqHeader::with_capacity(block_size as u64, quality, compress, true);
            let processor = VBinseqProcessor::new(header, policy, out_handle)?;

            // Process the records in parallel
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
        let (in_path1, in_path2) = args.input.paired_paths()?;
        encode_paired(
            in_path1,
            in_path2,
            args.output.borrowed_path(),
            args.output.mode()?,
            args.output.threads(),
            args.output.compress(),
            args.output.quality(),
            args.output.block_size,
            args.input.batch_size,
            args.output.policy.into(),
        )
    } else if args.input.interleaved {
        if let Some(FileFormat::Bam) = args.input.format {
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
        eprintln!("Wrote {num_records} records to {opath}");
    } else {
        eprintln!("Wrote {num_records} records");
    }
    if num_skipped > 0 {
        eprintln!("Skipped {num_skipped} records");
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

        eprintln!(
            "Distributing {} threads across {} files",
            num_threads,
            queue.len()
        );
        if leftover_threads > 0 {
            eprintln!(
                "Base threads per file: {base_threads_per_file}, extra threads for first {leftover_threads} file(s)"
            );
        } else {
            eprintln!("Threads per file: {base_threads_per_file}");
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
                eprintln!("Error in thread: {err:?}");
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
    eprintln!("Processing files in directory: {}", dir.display());

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
        pair_r1_r2_files(&fqueue)?
    } else {
        fqueue.into_iter().map(|f| vec![f]).collect()
    };

    if pqueue.is_empty() {
        bail!("No files found matching the expected pattern.");
    }

    if args.input.recursion.paired {
        eprintln!("Total file pairs found: {}", pqueue.len());
        // eprintln!("Pairs: {:#?}", pqueue);
    } else {
        eprintln!("Total files found: {}", pqueue.len());
        // eprintln!("Files: {:?}", pqueue);
    }

    process_queue(&args, pqueue, &regex)?;

    Ok(())
}

pub fn run(args: &EncodeCommand) -> Result<()> {
    if args.input.recursive {
        run_recursive(args)
    } else {
        run_atomic(args)
    }
}
