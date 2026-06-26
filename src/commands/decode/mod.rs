use std::io::Write;

mod decode_binseq;
mod utils;

use crate::cli::{DecodeCommand, Mate, OutputFile};
use decode_binseq::Decoder;
pub use utils::{write_record, write_record_pair, SplitWriter};

use anyhow::{bail, Result};
use binseq::prelude::*;
use log::{info, warn};

/// Convenience type wrapper
pub type Writer = Box<dyn Write + Send>;

pub fn build_writer(args: &OutputFile, paired: bool) -> Result<SplitWriter> {
    let format = args.format()?;

    // Split writer
    if args.prefix.is_some() {
        if !paired {
            bail!("Cannot split file into two. No extended sequence channel");
        }
        if args.mate == Mate::Both {
            let (r1, r2) = args.as_paired_writer(format)?;
            let split = SplitWriter::new_split(r1, r2);
            Ok(split)
        } else {
            // Interleaved writer
            let writer = args.as_writer()?;
            let split = SplitWriter::new_interleaved(writer);
            Ok(split)
        }
    } else {
        if !paired {
            match args.mate {
                Mate::One | Mate::Two => {
                    warn!("Ignoring `--mate/-m` flag as only single channel found in file");
                }
                Mate::Both => {}
            }
        }

        // Interleaved writer
        let writer = args.as_writer()?;
        let split = SplitWriter::new_interleaved(writer);
        Ok(split)
    }
}

pub fn run(args: &DecodeCommand) -> Result<()> {
    let reader = BinseqReader::new(args.input.path())?;
    let writer = build_writer(&args.output, reader.is_paired())?;
    let format = args.output.format()?;
    let mate = if reader.is_paired() {
        Some(args.output.mate())
    } else {
        None
    };
    let proc = Decoder::new(writer, format, mate);
    if let Some(mut span) = args.input.span {
        let num_records = reader.num_records()?;
        reader.process_parallel_range(
            proc.clone(),
            args.output.threads(),
            span.get_range(num_records)?,
        )?;
    } else {
        reader.process_parallel(proc.clone(), args.output.threads())?;
    }
    let num_records = proc.num_records();
    info!("Processed {num_records} records...");
    Ok(())
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use clap::Parser;
    use itertools::iproduct;
    use tempfile::NamedTempFile;

    use crate::cli::{BinseqMode, FileFormat};
    use crate::testutils::{count_binseq, count_fastx_records, write_fastx, Compression, DEFAULT_NUM_RECORDS};

    fn encode(in_path: &std::path::Path, out_path: &std::path::Path) -> Result<()> {
        let cmd = crate::cli::EncodeCommand::try_parse_from([
            "encode",
            in_path.to_str().unwrap(),
            "-o",
            out_path.to_str().unwrap(),
        ])?;
        crate::commands::encode::run(&cmd)
    }

    fn decode(bq_path: &std::path::Path, out_path: &std::path::Path) -> Result<()> {
        let cmd = crate::cli::DecodeCommand::try_parse_from([
            "decode",
            bq_path.to_str().unwrap(),
            "-o",
            out_path.to_str().unwrap(),
        ])?;
        super::run(&cmd)
    }

    /// Round-trip encode→decode preserves the exact record count across all mode/format/compression combos.
    #[test]
    fn test_round_trip_record_count() -> Result<()> {
        for (mode, fmt, comp) in iproduct!(
            BinseqMode::enum_iter(),
            FileFormat::fastx_iter(),
            Compression::all(),
        ) {
            let in_tmp = write_fastx().format(fmt).comp(comp).call()?;
            let bq_tmp = NamedTempFile::with_suffix(mode.extension())?;
            encode(in_tmp.path(), bq_tmp.path())?;

            let out_tmp = NamedTempFile::with_suffix(fmt.fastx_suffix())?;
            decode(bq_tmp.path(), out_tmp.path())?;

            let count = count_fastx_records(out_tmp.path())?;
            assert_eq!(
                count,
                DEFAULT_NUM_RECORDS,
                "round-trip record count wrong for {mode:?} {fmt:?} {comp:?}"
            );
        }
        Ok(())
    }

    #[test]
    fn test_decode_thread_counts() -> Result<()> {
        let in_tmp = write_fastx().nrec(1000).call()?;
        let bq_tmp = NamedTempFile::with_suffix(".cbq")?;
        encode(in_tmp.path(), bq_tmp.path())?;

        for threads in ["1", "2", "4"] {
            let out_tmp = NamedTempFile::with_suffix(".fastq")?;
            let cmd = crate::cli::DecodeCommand::try_parse_from([
                "decode",
                bq_tmp.path().to_str().unwrap(),
                "-o",
                out_tmp.path().to_str().unwrap(),
                "-T",
                threads,
            ])?;
            super::run(&cmd)?;
            assert_eq!(count_fastx_records(out_tmp.path())?, 1000);
        }
        Ok(())
    }

    #[test]
    fn test_decode_output_formats() -> Result<()> {
        let in_tmp = write_fastx().call()?;
        let bq_tmp = NamedTempFile::with_suffix(".cbq")?;
        encode(in_tmp.path(), bq_tmp.path())?;

        for (fmt_flag, out_suffix) in [("a", ".fasta"), ("q", ".fastq")] {
            let out_tmp = NamedTempFile::with_suffix(out_suffix)?;
            let cmd = crate::cli::DecodeCommand::try_parse_from([
                "decode",
                bq_tmp.path().to_str().unwrap(),
                "-o",
                out_tmp.path().to_str().unwrap(),
                "-f",
                fmt_flag,
            ])?;
            super::run(&cmd)?;
            assert_eq!(count_fastx_records(out_tmp.path())?, DEFAULT_NUM_RECORDS);
        }
        Ok(())
    }

    #[test]
    fn test_decode_paired_mate_selection() -> Result<()> {
        let r1 = write_fastx().call()?;
        let r2 = write_fastx().call()?;
        let bq_tmp = NamedTempFile::with_suffix(".cbq")?;
        let cmd = crate::cli::EncodeCommand::try_parse_from([
            "encode",
            r1.path().to_str().unwrap(),
            r2.path().to_str().unwrap(),
            "-o",
            bq_tmp.path().to_str().unwrap(),
        ])?;
        crate::commands::encode::run(&cmd)?;

        assert_eq!(count_binseq(bq_tmp.path())?, DEFAULT_NUM_RECORDS);

        for mate in ["1", "2", "both"] {
            let out_tmp = NamedTempFile::with_suffix(".fastq")?;
            let cmd = crate::cli::DecodeCommand::try_parse_from([
                "decode",
                bq_tmp.path().to_str().unwrap(),
                "-o",
                out_tmp.path().to_str().unwrap(),
                "-m",
                mate,
            ])?;
            super::run(&cmd)?;
        }
        Ok(())
    }
}
