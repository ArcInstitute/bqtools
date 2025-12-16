use std::{
    fs::File,
    io::{BufRead, BufReader},
    os::unix::fs::FileTypeExt,
    path::PathBuf,
};

use anyhow::{bail, Result};
use log::{debug, error, info, trace, warn};

use regex::Regex;
use walkdir::WalkDir;

#[cfg(feature = "htslib")]
use anyhow::Context;
#[cfg(feature = "htslib")]
use encode::encode_htslib;

use crate::{
    cli::{EncodeCommand, FileFormat},
    commands::encode::utils::{
        collate_groups, generate_output_name, pair_r1_r2_files, pull_single_files,
    },
};

mod encode;
pub mod processor;
pub mod utils;

use encode::encode_collection;

/// Run the encoding process for an atomic single/paired input
fn run_atomic(args: &EncodeCommand) -> Result<()> {
    let opath = args.output_path()?;
    let (num_records, num_skipped) = if args.input.paired() {
        trace!("launching paired encoding");
        encode_collection(
            args.input.build_paired_collection()?,
            opath.as_deref(),
            args.mode()?,
            args.output.clone().into(),
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
                encode_htslib(
                    args.input
                        .single_path()?
                        .context("Must provide an input path for HTSLib")?,
                    opath.as_deref(),
                    args.mode()?,
                    args.output.clone().into(),
                    true,
                )
            }
        } else {
            trace!("launching interleaved encoding (fastx)");
            encode_collection(
                args.input.build_interleaved_collection()?,
                opath.as_deref(),
                args.mode()?,
                args.output.clone().into(),
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
            encode_htslib(
                args.input
                    .single_path()?
                    .context("Must provide an input path for HTSlib")?,
                opath.as_deref(),
                args.mode()?,
                args.output.clone().into(),
                false,
            )
        }
    } else {
        trace!("launching single encoding (fastx)");
        encode_collection(
            args.input.build_single_collection()?,
            opath.as_deref(),
            args.mode()?,
            args.output.clone().into(),
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
                        let outpath = thread_args.output_path()?.ok_or_else(|| {
                            anyhow::anyhow!("Output path must be provided when collating files")
                        })?;

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

/// Build the regex pattern for filtering input files
fn build_file_regex(paired: bool) -> Result<Regex> {
    let regex_str = if paired {
        r"_R[12](_[^.]*)?\.(?:fastq|fq|fasta|fa)(?:\.gz|\.zst)?$"
    } else {
        r"\.(fastq|fq|fasta|fa)(\.gz|\.zst)?$"
    };
    Ok(Regex::new(regex_str)?)
}

/// Filter paths based on regex and file type (regular file or FIFO)
fn filter_valid_paths<I>(paths: I, regex: &Regex) -> Result<Vec<PathBuf>>
where
    I: Iterator<Item = PathBuf>,
{
    paths
        .filter(|path| {
            let path_str = path.to_string_lossy();
            if !regex.is_match(&path_str) {
                return false;
            }

            // Check if it's a regular file or FIFO
            match path.metadata() {
                Ok(metadata) => {
                    let file_type = metadata.file_type();
                    file_type.is_file() || file_type.is_fifo()
                }
                Err(_) => false,
            }
        })
        .collect::<Vec<_>>()
        .into_iter()
        .map(Ok)
        .collect()
}

/// Common logic for processing a list of file paths into a queue and executing
fn process_file_list(args: &EncodeCommand, file_queue: Vec<PathBuf>) -> Result<()> {
    if file_queue.is_empty() {
        bail!("No files found");
    }

    let mut sorted_queue = file_queue;
    sorted_queue.sort_unstable();

    // Build the regex for output naming
    let regex = build_file_regex(args.input.batch_encoding_options.paired)?;

    // Pair or pull single files
    let pqueue = if args.input.batch_encoding_options.paired {
        pair_r1_r2_files(&sorted_queue)
    } else {
        pull_single_files(&sorted_queue)
    }?;

    // Optionally collate
    let pqueue = if args.input.batch_encoding_options.collate {
        collate_groups(&pqueue)
    } else {
        pqueue
    };

    if pqueue.is_empty() {
        bail!("No files found matching the expected pattern.");
    }

    // Log what we found
    if args.input.batch_encoding_options.paired {
        info!("Total file pairs found: {}", pqueue.len());
    } else {
        info!("Total files found: {}", pqueue.len());
    }

    if pqueue.len() > 1 && args.output.output.is_some() {
        warn!("Output path specified but ignored when batch encoding multiple files.")
    }

    process_queue(args, pqueue, &regex)
}

fn run_recursive(args: &EncodeCommand) -> Result<()> {
    let args = args.to_owned();
    let dir = args.input.as_directory()?;

    info!("Processing files in directory: {}", dir.display());

    let regex = build_file_regex(args.input.batch_encoding_options.paired)?;

    let dir_walker = if let Some(max_depth) = args.input.recursion.depth {
        WalkDir::new(dir).max_depth(max_depth)
    } else {
        WalkDir::new(dir)
    };

    let file_queue = filter_valid_paths(
        dir_walker
            .into_iter()
            .filter_map(|e| e.ok())
            .map(|e| e.path().to_owned()),
        &regex,
    )?;

    process_file_list(&args, file_queue)
}

fn run_manifest(args: &EncodeCommand) -> Result<()> {
    let Some(manifest) = &args.input.manifest else {
        bail!("No manifest file provided");
    };

    let regex = build_file_regex(args.input.batch_encoding_options.paired)?;

    let handle = File::open(manifest).map(BufReader::new)?;
    let file_queue = filter_valid_paths(
        handle
            .lines()
            .filter_map(|line| line.ok())
            .map(PathBuf::from),
        &regex,
    )?;

    process_file_list(args, file_queue)
}

fn run_manifest_inline(args: &EncodeCommand) -> Result<()> {
    let regex = build_file_regex(args.input.batch_encoding_options.paired)?;

    let file_queue = filter_valid_paths(args.input.input.iter().map(PathBuf::from), &regex)?;

    process_file_list(args, file_queue)
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
