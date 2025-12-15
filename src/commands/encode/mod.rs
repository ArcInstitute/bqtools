use std::{
    fs::File,
    io::{BufRead, BufReader},
    os::unix::fs::FileTypeExt,
    path::PathBuf,
};

use anyhow::{bail, Result};
use binseq::{bq, vbq, BitSize, Policy};
use log::{debug, error, info, trace};
use paraseq::{
    fastx::{self, Format},
    prelude::*,
};

use regex::Regex;
use walkdir::WalkDir;

#[cfg(feature = "htslib")]
use crate::commands::encode::utils::get_sequence_len_htslib;
#[cfg(feature = "htslib")]
use anyhow::Context;
#[cfg(feature = "htslib")]
use paraseq::htslib;

use crate::{
    cli::{BinseqMode, EncodeCommand, FileFormat},
    commands::{
        encode::utils::{
            collate_groups, generate_output_name, get_interleaved_sequence_len, get_sequence_len,
            pair_r1_r2_files, pull_single_files,
        },
        utils::match_output,
    },
    types::BoxedReader,
};

pub mod processor;
pub mod utils;

use processor::{BinseqProcessor, VBinseqProcessor};

#[allow(clippy::too_many_arguments)]
fn encode_single(
    mut collection: fastx::Collection<BoxedReader>,
    out_path: Option<&str>,
    mode: BinseqMode,
    num_threads: usize,
    compress: bool,
    quality: bool,
    block_size: usize,
    policy: Policy,
    bitsize: BitSize,
    headers: bool,
) -> Result<(usize, usize)> {
    // build writer
    let out_handle = match_output(out_path)?;

    let (num_records, num_skipped) = if mode == BinseqMode::Binseq {
        trace!("converting to bq");

        // Determine the sequence length
        let inner = collection.inner_mut();
        let slen = get_sequence_len(&mut inner[0])?;
        trace!("sequence length: {slen}");

        let header = bq::BinseqHeaderBuilder::new()
            .slen(slen)
            .bitsize(bitsize)
            .flags(false)
            .build()?;
        let mut processor = BinseqProcessor::new(header, policy, out_handle)?;

        // Process the records in parallel
        trace!("processing records in parallel (T={num_threads})");
        collection.process_parallel(&mut processor, num_threads, None)?;

        // Update the number of records
        let num_records = processor.get_global_record_count();
        let num_skipped = processor.get_global_skipped_count();

        (num_records, num_skipped)
    } else {
        trace!("converting to vbq");
        let quality = match collection.unique_format() {
            Some(Format::Fastq) | None => quality,
            Some(Format::Fasta) => false, // never record fasta quality
        };
        trace!("quality: {quality}");
        let header = vbq::VBinseqHeaderBuilder::new()
            .block(block_size as u64)
            .qual(quality)
            .compressed(compress)
            .bitsize(bitsize)
            .headers(headers)
            .flags(false)
            .build();
        let mut processor = VBinseqProcessor::new(header, policy, out_handle)?;

        // Process the records in parallel
        trace!("processing records in parallel (T={num_threads})");
        collection.process_parallel(&mut processor, num_threads, None)?;
        processor.finish()?;

        // Update the number of records
        let num_records = processor.get_global_record_count();
        let num_skipped = processor.get_global_skipped_count();

        (num_records, num_skipped)
    };

    Ok((num_records, num_skipped))
}

#[cfg(feature = "htslib")]
#[allow(clippy::too_many_arguments)]
fn encode_single_htslib(
    in_path: &str,
    out_path: Option<&str>,
    mode: BinseqMode,
    num_threads: usize,
    compress: bool,
    quality: bool,
    block_size: usize,
    policy: Policy,
    bitsize: BitSize,
    headers: bool,
) -> Result<(usize, usize)> {
    // build reader
    let reader = htslib::Reader::from_path(in_path)?;

    // build writer
    let out_handle = match_output(out_path)?;

    let (num_records, num_skipped) = if mode == BinseqMode::Binseq {
        trace!("converting to bq");

        // Determine the sequence length
        let (slen, _) = get_sequence_len_htslib(in_path, false)?;
        trace!("sequence length: {slen}");

        let header = bq::BinseqHeaderBuilder::new()
            .slen(slen)
            .bitsize(bitsize)
            .flags(false)
            .build()?;
        let mut processor = BinseqProcessor::new(header, policy, out_handle)?;

        // Process the records in parallel
        trace!("processing records in parallel (T={num_threads})");
        reader.process_parallel(&mut processor, num_threads)?;

        // Update the number of records
        let num_records = processor.get_global_record_count();
        let num_skipped = processor.get_global_skipped_count();

        (num_records, num_skipped)
    } else {
        trace!("converting to vbq");
        let header = vbq::VBinseqHeaderBuilder::new()
            .block(block_size as u64)
            .qual(quality)
            .compressed(compress)
            .bitsize(bitsize)
            .headers(headers)
            .flags(false)
            .build();
        let mut processor = VBinseqProcessor::new(header, policy, out_handle)?;

        // Process the records in parallel
        trace!("processing records in parallel (T={num_threads})");
        reader.process_parallel(&mut processor, num_threads)?;
        processor.finish()?;

        // Update the number of records
        let num_records = processor.get_global_record_count();
        let num_skipped = processor.get_global_skipped_count();

        (num_records, num_skipped)
    };

    Ok((num_records, num_skipped))
}

#[allow(clippy::too_many_arguments)]
fn encode_interleaved(
    mut collection: fastx::Collection<BoxedReader>,
    out_path: Option<&str>,
    mode: BinseqMode,
    num_threads: usize,
    compress: bool,
    quality: bool,
    block_size: usize,
    policy: Policy,
    bitsize: BitSize,
    headers: bool,
) -> Result<(usize, usize)> {
    // Prepare the processor
    let out_handle = match_output(out_path)?;

    let (num_records, num_skipped) = if mode == BinseqMode::Binseq {
        trace!("converting to bq");

        // Determine the sequence length
        let inner = collection.inner_mut();
        let (slen, xlen) = get_interleaved_sequence_len(&mut inner[0])?;
        trace!("sequence length: slen={slen}, xlen={xlen}");

        let header = bq::BinseqHeaderBuilder::new()
            .slen(slen)
            .xlen(xlen)
            .bitsize(bitsize)
            .flags(false)
            .build()?;
        let mut processor = BinseqProcessor::new(header, policy, out_handle)?;

        // Process the records in parallel
        trace!("processing records in parallel (T={num_threads})");
        collection.process_parallel_interleaved(&mut processor, num_threads, None)?;

        // Update the number of records
        let num_records = processor.get_global_record_count();
        let num_skipped = processor.get_global_skipped_count();

        (num_records, num_skipped)
    } else {
        trace!("converting to vbq");

        let quality = match collection.unique_format() {
            Some(Format::Fastq) | None => quality,
            Some(Format::Fasta) => false, // never record quality for fasta
        };
        trace!("quality: {quality}");
        let header = vbq::VBinseqHeaderBuilder::new()
            .block(block_size as u64)
            .qual(quality)
            .compressed(compress)
            .paired(true)
            .bitsize(bitsize)
            .headers(headers)
            .flags(false)
            .build();
        let mut processor = VBinseqProcessor::new(header, policy, out_handle)?;

        // Process the records in parallel
        trace!("processing records in parallel (T={num_threads})");
        collection.process_parallel_interleaved(&mut processor, num_threads, None)?;
        processor.finish()?;

        // Update the number of records
        let num_records = processor.get_global_record_count();
        let num_skipped = processor.get_global_skipped_count();

        (num_records, num_skipped)
    };

    Ok((num_records, num_skipped))
}

#[cfg(feature = "htslib")]
#[allow(clippy::too_many_arguments)]
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
    bitsize: BitSize,
    headers: bool,
) -> Result<(usize, usize)> {
    let reader = htslib::Reader::from_path(in_path)?;

    // Prepare the processor
    let out_handle = match_output(out_path)?;

    let (num_records, num_skipped) = if mode == BinseqMode::Binseq {
        trace!("converting to bq");

        // Determine the sequence length
        let (slen, xlen) = get_sequence_len_htslib(in_path, true)?;
        trace!("sequence length: {slen}, xlen: {xlen}");

        let header = bq::BinseqHeaderBuilder::new()
            .slen(slen)
            .xlen(xlen)
            .bitsize(bitsize)
            .flags(false)
            .build()?;
        let mut processor = BinseqProcessor::new(header, policy, out_handle)?;

        // Process the records in parallel
        trace!("processing records in parallel (T={num_threads})");
        reader.process_parallel_interleaved(&mut processor, num_threads)?;

        // Update the number of records
        let num_records = processor.get_global_record_count();
        let num_skipped = processor.get_global_skipped_count();

        (num_records, num_skipped)
    } else {
        trace!("converting to vbq");
        let header = vbq::VBinseqHeaderBuilder::new()
            .block(block_size as u64)
            .qual(quality)
            .compressed(compress)
            .paired(true)
            .bitsize(bitsize)
            .headers(headers)
            .flags(false)
            .build();
        let mut processor = VBinseqProcessor::new(header, policy, out_handle)?;

        // Process the records in parallel
        trace!("processing records in parallel (T={num_threads})");
        reader.process_parallel_interleaved(&mut processor, num_threads)?;
        processor.finish()?;

        // Update the number of records
        let num_records = processor.get_global_record_count();
        let num_skipped = processor.get_global_skipped_count();

        (num_records, num_skipped)
    };

    Ok((num_records, num_skipped))
}

#[allow(clippy::too_many_arguments)]
fn encode_paired(
    mut collection: fastx::Collection<BoxedReader>,
    out_path: Option<&str>,
    mode: BinseqMode,
    num_threads: usize,
    compress: bool,
    quality: bool,
    block_size: usize,
    policy: Policy,
    bitsize: BitSize,
    headers: bool,
) -> Result<(usize, usize)> {
    // Prepare the output handle
    let out_handle = match_output(out_path)?;

    let (num_records, num_skipped) = match mode {
        BinseqMode::Binseq => {
            trace!("converting to bq");

            // Determine the sequence length
            let inner = collection.inner_mut();
            let slen = get_sequence_len(&mut inner[0])?;
            let xlen = get_sequence_len(&mut inner[1])?;
            trace!("sequence length: slen={slen}, xlen={xlen}");

            // Prepare the output HEADER
            let header = bq::BinseqHeaderBuilder::new()
                .slen(slen)
                .xlen(xlen)
                .bitsize(bitsize)
                .flags(false)
                .build()?;
            let mut processor = BinseqProcessor::new(header, policy, out_handle)?;

            // Process the records in parallel
            trace!("processing records in parallel (T={num_threads})");
            collection.process_parallel_paired(&mut processor, num_threads, None)?;

            // Update the number of records
            let num_records = processor.get_global_record_count();
            let num_skipped = processor.get_global_skipped_count();

            (num_records, num_skipped)
        }
        BinseqMode::VBinseq => {
            trace!("converting to vbq");

            let quality = match collection.unique_format() {
                Some(Format::Fastq) | None => quality,
                Some(Format::Fasta) => false, // never record quality for fasta
            };

            trace!("quality: {quality}");
            let header = vbq::VBinseqHeaderBuilder::new()
                .block(block_size as u64)
                .qual(quality)
                .compressed(compress)
                .paired(true)
                .bitsize(bitsize)
                .headers(headers)
                .flags(false)
                .build();
            let mut processor = VBinseqProcessor::new(header, policy, out_handle)?;

            // Process the records in parallel
            trace!("processing records in parallel (T={num_threads})");
            collection.process_parallel_paired(&mut processor, num_threads, None)?;
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
    let opath = args.output_path()?;
    let (num_records, num_skipped) = if args.input.paired() {
        trace!("launching paired encoding");
        let collection = args.input.build_paired_collection()?;
        encode_paired(
            collection,
            opath.as_deref(),
            args.mode()?,
            args.output.threads(),
            args.output.compress(),
            args.output.quality(),
            args.output.block_size(),
            args.output.policy.into(),
            args.output.bitsize(),
            args.output.headers,
        )
    } else if args.input.interleaved {
        if let Some(FileFormat::Bam) = args.input.format() {
            #[cfg(not(feature = "htslib"))]
            {
                error!("Missing feature flag - htslib. Please compile with htslib feature flag enabled to process HTSlib files");
                bail!("Missing feature flag - htslib");
            }

            #[cfg(feature = "htslib")]
            {
                trace!("launching interleaved encoding (htslib)");
                encode_interleaved_htslib(
                    args.input
                        .single_path()?
                        .context("Must provide an input path for HTSLib")?,
                    opath.as_deref(),
                    args.mode()?,
                    args.output.threads(),
                    args.output.compress(),
                    args.output.quality(),
                    args.output.block_size(),
                    args.input.batch_size,
                    args.output.policy.into(),
                    args.output.bitsize(),
                    args.output.headers,
                )
            }
        } else {
            trace!("launching interleaved encoding (fastx)");
            encode_interleaved(
                args.input.build_interleaved_collection()?,
                opath.as_deref(),
                args.mode()?,
                args.output.threads(),
                args.output.compress(),
                args.output.quality(),
                args.output.block_size(),
                args.output.policy.into(),
                args.output.bitsize(),
                args.output.headers,
            )
        }
    } else if let Some(FileFormat::Bam) = args.input.format() {
        #[cfg(not(feature = "htslib"))]
        {
            error!("Missing feature flag - htslib. Please compile with htslib feature flag enabled to process HTSlib files");
            bail!("Missing feature flag - htslib");
        }

        #[cfg(feature = "htslib")]
        {
            trace!("launching single encoding (htslib)");
            encode_single_htslib(
                args.input
                    .single_path()?
                    .context("Must provide an input path for HTSlib")?,
                opath.as_deref(),
                args.mode()?,
                args.output.threads(),
                args.output.compress(),
                args.output.quality(),
                args.output.block_size(),
                args.output.policy.into(),
                args.output.bitsize(),
                args.output.headers,
            )
        }
    } else {
        trace!("launching single encoding (fastx)");
        encode_single(
            args.input.build_single_collection()?,
            opath.as_deref(),
            args.mode()?,
            args.output.threads(),
            args.output.compress(),
            args.output.quality(),
            args.output.block_size(),
            args.output.policy.into(),
            args.output.bitsize(),
            args.output.headers,
        )
    }?;

    if let Some(opath) = opath {
        info!("Wrote {num_records} records to: {opath}");
    } else {
        info!("Wrote {num_records} records to: stdout");
    }
    if num_skipped > 0 {
        info!("Skipped {num_skipped} records");
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
            let mode = args.mode()?;

            // First `leftover_threads` files get one extra thread
            let threads_for_this_file = if i < leftover_threads {
                base_threads_per_file + 1
            } else {
                base_threads_per_file
            };

            let handle = std::thread::spawn(move || -> Result<()> {
                let mut file_args = thread_args.clone();

                let outpath = match pair.len() {
                    1 => {
                        let inpath = pair[0].to_str().unwrap().to_string();
                        let outpath = thread_regex
                            .replace_all(&inpath, mode.extension())
                            .to_string();
                        file_args.input.input = vec![inpath];
                        file_args.output.output = Some(outpath.clone());
                        file_args.output.threads = threads_for_this_file;
                        outpath
                    }
                    2 => {
                        let inpaths: Vec<String> = pair
                            .iter()
                            .map(|path| path.to_str().unwrap().to_string())
                            .collect();
                        let outpath = generate_output_name(&pair, mode.extension())?;

                        file_args.input.input = inpaths;
                        file_args.output.output = Some(outpath.clone());
                        file_args.output.threads = threads_for_this_file;
                        outpath
                    }
                    _ => {
                        let inpaths: Vec<String> = pair
                            .iter()
                            .map(|path| path.to_str().unwrap().to_string())
                            .collect();
                        let outpath = thread_args
                            .output_path()?
                            .expect("Failed to generate output path");

                        file_args.input.input = inpaths;
                        file_args.output.output = Some(outpath.clone());
                        file_args.output.threads = threads_for_this_file;
                        outpath
                    }
                };

                match run_atomic(&file_args) {
                    Ok(()) => (),
                    Err(err) => {
                        error!("Error generating output: {outpath}\n{err:?}\nSkipping.");
                        trace!("Removing partial file: {outpath}");
                        std::fs::remove_file(outpath)?;
                    }
                }
                Ok(())
            });
            handles.push(handle);
        }

        for handle in handles {
            match handle.join() {
                Ok(res) => match res {
                    Ok(()) => (),
                    Err(err) => error!("Error in thread: {err:?}"),
                },
                Err(err) => error!("Error joining thread: {err:?}"),
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

    let regex_str = if args.input.batch_encoding_options.paired {
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
        if regex.is_match(path_str) && (path.metadata()?.file_type().is_fifo() || path.is_file()) {
            fqueue.push(path.to_owned());
        }
    }
    if fqueue.is_empty() {
        bail!("No files found");
    }
    fqueue.sort_unstable();

    let pqueue = if args.input.batch_encoding_options.paired {
        pair_r1_r2_files(&fqueue)
    } else {
        pull_single_files(&fqueue)
    }?;

    let pqueue = if args.input.batch_encoding_options.collate {
        collate_groups(&pqueue)
    } else {
        pqueue
    };

    if pqueue.is_empty() {
        bail!("No files found matching the expected pattern.");
    }

    if args.input.batch_encoding_options.paired {
        info!("Total file pairs found: {}", pqueue.len());
    } else {
        info!("Total files found: {}", pqueue.len());
    }

    process_queue(&args, pqueue, &regex)?;

    Ok(())
}

fn run_manifest(args: &EncodeCommand) -> Result<()> {
    let Some(manifest) = &args.input.manifest else {
        bail!("No manifest file provided");
    };

    let regex_str = if args.input.batch_encoding_options.paired {
        r"_R[12](_[^.]*)?\.(?:fastq|fq|fasta|fa)(?:\.gz|\.zst)?$"
    } else {
        r"\.(fastq|fq|fasta|fa)(\.gz|\.zst)?$"
    };
    let regex = Regex::new(regex_str)?;

    let handle = File::open(manifest).map(BufReader::new)?;
    let mut fqueue = vec![];
    for line in handle.lines() {
        let line = line?;
        let path = PathBuf::from(line);
        let path_str = path.as_os_str().to_str().unwrap();
        if regex.is_match(path_str) && (path.metadata()?.file_type().is_fifo() || path.is_file()) {
            fqueue.push(path);
        }
    }
    if fqueue.is_empty() {
        bail!("No files found");
    }
    fqueue.sort_unstable();

    let pqueue = if args.input.batch_encoding_options.paired {
        pair_r1_r2_files(&fqueue)
    } else {
        pull_single_files(&fqueue)
    }?;

    let pqueue = if args.input.batch_encoding_options.collate {
        collate_groups(&pqueue)
    } else {
        pqueue
    };

    if pqueue.is_empty() {
        bail!("No files found matching the expected pattern.");
    }

    if args.input.batch_encoding_options.paired {
        info!("Total file pairs found: {}", pqueue.len());
    } else {
        info!("Total files found: {}", pqueue.len());
    }

    process_queue(args, pqueue, &regex)?;

    Ok(())
}

fn run_manifest_inline(args: &EncodeCommand) -> Result<()> {
    let regex_str = if args.input.batch_encoding_options.paired {
        r"_R[12](_[^.]*)?\.(?:fastq|fq|fasta|fa)(?:\.gz|\.zst)?$"
    } else {
        r"\.(fastq|fq|fasta|fa)(\.gz|\.zst)?$"
    };
    let regex = Regex::new(regex_str)?;

    let mut fqueue = vec![];
    for path in &args.input.input {
        let path = PathBuf::from(path);
        let path_str = path.as_os_str().to_str().unwrap();
        if regex.is_match(path_str) && (path.metadata()?.file_type().is_fifo() || path.is_file()) {
            fqueue.push(path);
        }
    }
    if fqueue.is_empty() {
        bail!("No files found");
    }
    fqueue.sort_unstable();

    let pqueue = if args.input.batch_encoding_options.paired {
        pair_r1_r2_files(&fqueue)
    } else {
        pull_single_files(&fqueue)
    }?;

    let pqueue = if args.input.batch_encoding_options.collate {
        collate_groups(&pqueue)
    } else {
        pqueue
    };

    if pqueue.is_empty() {
        bail!("No files found matching the expected pattern.");
    }

    if args.input.batch_encoding_options.paired {
        info!("Total file pairs found: {}", pqueue.len());
    } else {
        info!("Total files found: {}", pqueue.len());
    }

    process_queue(args, pqueue, &regex)?;

    Ok(())
}

pub fn run(args: &EncodeCommand) -> Result<()> {
    if args.input.recursive {
        trace!("launching encode-recursive");
        run_recursive(args)
    } else if args.input.manifest.is_some() {
        trace!("launching encode-manifest");
        run_manifest(args)
    } else if args.input.num_files() > 2 {
        trace!("launching inline manifest");
        run_manifest_inline(args)
    } else {
        trace!("launching encode-atomic");
        run_atomic(args)
    }
}
