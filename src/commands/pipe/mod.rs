pub mod exec;
pub mod processor;
pub mod utils;

use std::io::Write;
use std::thread;

use anyhow::Result;
use binseq::BinseqReader;
use log::{info, warn};

use crate::cli::{FileFormat, PipeCommand};
use exec::ExecMode;
use processor::PipeProcessor;
use utils::{create_fifos, FifoGuard};

pub type BoxedWriter = Box<dyn Write + Send>;

/// Simple enum to represent the type of record pair to process.
#[derive(Clone, Copy, Debug)]
pub enum RecordPair {
    R1,
    R2,
    Unpaired,
}

/// Which channels to create FIFOs and writer threads for in paired mode.
///
/// Derived from the exec template: a template with only `{R1}` suppresses R2
/// entirely so no unread FIFO is left open. Only meaningful for paired files;
/// unpaired files always write a single unlabelled FIFO.
#[derive(Clone, Copy, Debug)]
pub enum PairedChannels {
    Both,
    R1Only,
    R2Only,
}

pub fn run(args: &PipeCommand) -> Result<()> {
    if args.input.span.is_some() {
        warn!("Span is ignored when using pipe subcommand");
    }

    let format = args.format()?;
    let reader = BinseqReader::new(args.input.path())?;
    let num_records = reader.num_records()?;
    let paired = reader.is_paired();
    let num_pipes = if paired {
        (args.num_pipes() / 2).max(1)
    } else {
        args.num_pipes()
    };

    // Validate exec templates before creating FIFOs so a bad template fails fast
    // rather than leaving an open FIFO with no reader (which would hang).
    if let Some(t) = args.exec().or_else(|| args.exec_batch()) {
        exec::validate_template(t, paired)?;
    }

    // Determine which channels to create FIFOs and writer threads for. In exec
    // mode the template drives this — a template with only {R1} skips the R2
    // FIFO and writer entirely. Without exec, both channels are always created.
    // Only meaningful for paired files; unpaired always uses a single unlabelled FIFO.
    let channels = if paired {
        args.exec()
            .or_else(|| args.exec_batch())
            .map_or(PairedChannels::Both, exec::required_channels)
    } else {
        PairedChannels::Both
    };

    let basename = args.basepath();
    // Wrap the FIFOs in a guard immediately so they are unlinked on any early
    // return or panic below, not just on the happy path.
    let fifo_guard = FifoGuard::new(create_fifos(basename, paired, num_pipes, format, channels)?);
    info!(
        "{} FIFOs created. Waiting for readers to connect...",
        fifo_guard.paths().len()
    );

    let records_per_pipe = num_records / num_pipes;

    // Spawn consumer processes before writer threads: opening a FIFO for writing
    // blocks until a reader connects, so readers must be in-flight first.
    let exec_mode = if let Some(t) = args.exec() {
        Some(ExecMode::PerFifo(t))
    } else {
        args.exec_batch().map(ExecMode::Batch)
    };
    let mut consumers = match exec_mode {
        Some(mode) => exec::spawn_consumers(mode, basename, paired, num_pipes, format)?,
        None => Vec::new(),
    };

    // For each pipe, open a thread which handles the init and exit of the writer.
    // Named pipes block on open until both reader and writer connect.
    let mut handles = Vec::new();
    for pid in 0..num_pipes {
        let rstart = records_per_pipe * pid;
        let rend = if pid == num_pipes - 1 {
            num_records
        } else {
            rstart + records_per_pipe
        };

        if paired {
            if matches!(channels, PairedChannels::Both | PairedChannels::R1Only) {
                handles.push(spawn_pipe_thread(
                    basename.to_string(),
                    args.input.path().to_string(),
                    pid,
                    format,
                    RecordPair::R1,
                    rstart..rend,
                ));
            }
            if matches!(channels, PairedChannels::Both | PairedChannels::R2Only) {
                handles.push(spawn_pipe_thread(
                    basename.to_string(),
                    args.input.path().to_string(),
                    pid,
                    format,
                    RecordPair::R2,
                    rstart..rend,
                ));
            }
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

    for handle in handles {
        handle.join().unwrap()?;
    }

    for child in &mut consumers {
        let status = child.wait()?;
        if !status.success() {
            anyhow::bail!("exec command failed with status: {status}");
        }
    }

    // `fifo_guard` unlinks the FIFOs as it drops here (and on any early return above).
    info!("Closing FIFOs");
    drop(fifo_guard);

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

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use clap::Parser;
    use tempfile::NamedTempFile;

    use crate::testutils::{count_fastx_records, write_fastx, DEFAULT_NUM_RECORDS};

    fn encode(in_path: &std::path::Path, out_path: &std::path::Path) -> Result<()> {
        let cmd = crate::cli::EncodeCommand::try_parse_from([
            "encode",
            in_path.to_str().unwrap(),
            "-o",
            out_path.to_str().unwrap(),
        ])?;
        crate::commands::encode::run(&cmd)
    }

    /// Single-end pipe with `-x`: one command per FIFO, `{}` substituted with the path.
    #[test]
    fn test_pipe_exec_single() -> Result<()> {
        let fastq = write_fastx().call()?;
        let cbq = NamedTempFile::with_suffix(".cbq")?;
        encode(fastq.path(), cbq.path())?;

        let fifo_dir = tempfile::tempdir()?;
        let basepath = fifo_dir.path().join("pipe").to_str().unwrap().to_string();
        let out = NamedTempFile::with_suffix(".fastq")?;
        let out_path = out.path().to_str().unwrap().to_string();

        let cmd = crate::cli::PipeCommand::try_parse_from([
            "pipe",
            cbq.path().to_str().unwrap(),
            "-b",
            &basepath,
            "-p",
            "1",
            "-x",
            &format!("cat {{}} > {out_path}"),
        ])?;
        super::run(&cmd)?;

        assert_eq!(
            count_fastx_records(out.path())?,
            DEFAULT_NUM_RECORDS,
            "record count mismatch for single-end -x"
        );
        Ok(())
    }

    /// Single-end pipe with `-X`: one command receives all FIFO paths space-joined via `{}`.
    #[test]
    fn test_pipe_exec_batch_single() -> Result<()> {
        let fastq = write_fastx().call()?;
        let cbq = NamedTempFile::with_suffix(".cbq")?;
        encode(fastq.path(), cbq.path())?;

        let fifo_dir = tempfile::tempdir()?;
        let basepath = fifo_dir.path().join("pipe").to_str().unwrap().to_string();
        let out = NamedTempFile::with_suffix(".fastq")?;
        let out_path = out.path().to_str().unwrap().to_string();

        let cmd = crate::cli::PipeCommand::try_parse_from([
            "pipe",
            cbq.path().to_str().unwrap(),
            "-b",
            &basepath,
            "-p",
            "2",
            "-X",
            &format!("cat {{}} > {out_path}"),
        ])?;
        super::run(&cmd)?;

        assert_eq!(
            count_fastx_records(out.path())?,
            DEFAULT_NUM_RECORDS,
            "record count mismatch for single-end -X"
        );
        Ok(())
    }

    /// Paired-end pipe with `-x`: one command per pair, `{R1}` and `{R2}` substituted.
    #[test]
    fn test_pipe_exec_paired() -> Result<()> {
        let r1 = write_fastx().call()?;
        let r2 = write_fastx().call()?;
        let cbq = NamedTempFile::with_suffix(".cbq")?;
        let encode_cmd = crate::cli::EncodeCommand::try_parse_from([
            "encode",
            r1.path().to_str().unwrap(),
            r2.path().to_str().unwrap(),
            "-o",
            cbq.path().to_str().unwrap(),
        ])?;
        crate::commands::encode::run(&encode_cmd)?;

        let fifo_dir = tempfile::tempdir()?;
        let basepath = fifo_dir.path().join("pipe").to_str().unwrap().to_string();
        let r1_out = NamedTempFile::with_suffix(".fastq")?;
        let r2_out = NamedTempFile::with_suffix(".fastq")?;
        let r1_path = r1_out.path().to_str().unwrap().to_string();
        let r2_path = r2_out.path().to_str().unwrap().to_string();

        let cmd = crate::cli::PipeCommand::try_parse_from([
            "pipe",
            cbq.path().to_str().unwrap(),
            "-b",
            &basepath,
            "-p",
            "2",
            "-x",
            &format!("cat {{R1}} > {r1_path} && cat {{R2}} > {r2_path}"),
        ])?;
        super::run(&cmd)?;

        assert_eq!(
            count_fastx_records(r1_out.path())?,
            DEFAULT_NUM_RECORDS,
            "R1 record count mismatch for paired -x"
        );
        assert_eq!(
            count_fastx_records(r2_out.path())?,
            DEFAULT_NUM_RECORDS,
            "R2 record count mismatch for paired -x"
        );
        Ok(())
    }

    /// Paired-end pipe with `-X` and adjacent `{R1} {R2}`: paths are interleaved
    /// (r1_0 r2_0 r1_1 r2_1 …) so positional-argument tools receive pairs together.
    #[test]
    fn test_pipe_exec_batch_paired_interleaved() -> Result<()> {
        let r1 = write_fastx().call()?;
        let r2 = write_fastx().call()?;
        let cbq = NamedTempFile::with_suffix(".cbq")?;
        let encode_cmd = crate::cli::EncodeCommand::try_parse_from([
            "encode",
            r1.path().to_str().unwrap(),
            r2.path().to_str().unwrap(),
            "-o",
            cbq.path().to_str().unwrap(),
        ])?;
        crate::commands::encode::run(&encode_cmd)?;

        let fifo_dir = tempfile::tempdir()?;
        let basepath = fifo_dir.path().join("pipe").to_str().unwrap().to_string();
        let out = NamedTempFile::with_suffix(".fastq")?;
        let out_path = out.path().to_str().unwrap().to_string();

        // {R1} {R2} adjacent → interleaved expansion; `cat` reads r1_0 r2_0 r1_1 r2_1 …
        let cmd = crate::cli::PipeCommand::try_parse_from([
            "pipe",
            cbq.path().to_str().unwrap(),
            "-b",
            &basepath,
            "-p",
            "2",
            "-X",
            &format!("cat {{R1}} {{R2}} > {out_path}"),
        ])?;
        super::run(&cmd)?;

        // Each pair contributes one R1 + one R2 record, so total = 2 × N.
        assert_eq!(
            count_fastx_records(out.path())?,
            DEFAULT_NUM_RECORDS * 2,
            "interleaved paired -X record count mismatch"
        );
        Ok(())
    }

    /// Missing `{}` in a single-end template must be caught before any FIFO is created.
    #[test]
    fn test_pipe_exec_missing_token_single() -> Result<()> {
        let fastq = write_fastx().call()?;
        let cbq = NamedTempFile::with_suffix(".cbq")?;
        encode(fastq.path(), cbq.path())?;

        let fifo_dir = tempfile::tempdir()?;
        let basepath = fifo_dir.path().join("pipe").to_str().unwrap().to_string();

        let cmd = crate::cli::PipeCommand::try_parse_from([
            "pipe",
            cbq.path().to_str().unwrap(),
            "-b",
            &basepath,
            "-x",
            "cat /dev/null",
        ])?;
        assert!(
            super::run(&cmd).is_err(),
            "should error when {{}} is missing from single-end template"
        );
        Ok(())
    }

    /// A paired template with neither `{R1}` nor `{R2}` must be caught before any FIFO is created.
    #[test]
    fn test_pipe_exec_missing_token_paired() -> Result<()> {
        let r1 = write_fastx().call()?;
        let r2 = write_fastx().call()?;
        let cbq = NamedTempFile::with_suffix(".cbq")?;
        let encode_cmd = crate::cli::EncodeCommand::try_parse_from([
            "encode",
            r1.path().to_str().unwrap(),
            r2.path().to_str().unwrap(),
            "-o",
            cbq.path().to_str().unwrap(),
        ])?;
        crate::commands::encode::run(&encode_cmd)?;

        let fifo_dir = tempfile::tempdir()?;
        let basepath = fifo_dir.path().join("pipe").to_str().unwrap().to_string();

        // Template has neither {R1} nor {R2} — nothing to read from either FIFO.
        let cmd = crate::cli::PipeCommand::try_parse_from([
            "pipe",
            cbq.path().to_str().unwrap(),
            "-b",
            &basepath,
            "-x",
            "cat /dev/null",
        ])?;
        assert!(
            super::run(&cmd).is_err(),
            "should error when neither {{R1}} nor {{R2}} is present in paired template"
        );
        Ok(())
    }

    /// `{n}` expands to the pipe index, letting users route per-shard output to
    /// separate files without stdout collisions.
    #[test]
    fn test_pipe_exec_pipe_index_token() -> Result<()> {
        let fastq = write_fastx().call()?;
        let cbq = NamedTempFile::with_suffix(".cbq")?;
        encode(fastq.path(), cbq.path())?;

        let fifo_dir = tempfile::tempdir()?;
        let basepath = fifo_dir.path().join("pipe").to_str().unwrap().to_string();
        let out_dir = tempfile::tempdir()?;
        let out_prefix = out_dir.path().join("shard").to_str().unwrap().to_string();

        // Each shard writes to its own file: shard_0.fastq, shard_1.fastq
        let cmd = crate::cli::PipeCommand::try_parse_from([
            "pipe",
            cbq.path().to_str().unwrap(),
            "-b",
            &basepath,
            "-p",
            "2",
            "-x",
            &format!("cat {{}} > {out_prefix}_{{n}}.fastq"),
        ])?;
        super::run(&cmd)?;

        let shard0 = out_dir.path().join("shard_0.fastq");
        let shard1 = out_dir.path().join("shard_1.fastq");
        let total = count_fastx_records(&shard0)? + count_fastx_records(&shard1)?;
        assert_eq!(total, DEFAULT_NUM_RECORDS, "sharded record total mismatch");
        Ok(())
    }

    /// Paired-end pipe with `-x` using only `{R1}`: only R1 FIFOs are created and
    /// written; R2 is silently skipped. This is valid — one-mate-only processing.
    #[test]
    fn test_pipe_exec_paired_r1_only() -> Result<()> {
        let r1 = write_fastx().call()?;
        let r2 = write_fastx().call()?;
        let cbq = NamedTempFile::with_suffix(".cbq")?;
        let encode_cmd = crate::cli::EncodeCommand::try_parse_from([
            "encode",
            r1.path().to_str().unwrap(),
            r2.path().to_str().unwrap(),
            "-o",
            cbq.path().to_str().unwrap(),
        ])?;
        crate::commands::encode::run(&encode_cmd)?;

        let fifo_dir = tempfile::tempdir()?;
        let basepath = fifo_dir.path().join("pipe").to_str().unwrap().to_string();
        let r1_out = NamedTempFile::with_suffix(".fastq")?;
        let r1_path = r1_out.path().to_str().unwrap().to_string();

        let cmd = crate::cli::PipeCommand::try_parse_from([
            "pipe",
            cbq.path().to_str().unwrap(),
            "-b",
            &basepath,
            "-p",
            "1",
            "-x",
            &format!("cat {{R1}} > {r1_path}"),
        ])?;
        super::run(&cmd)?;

        assert_eq!(
            count_fastx_records(r1_out.path())?,
            DEFAULT_NUM_RECORDS,
            "R1-only paired -x record count mismatch"
        );
        Ok(())
    }

    /// Paired-end pipe with `-x` using only `{R2}`: only R2 FIFOs are created and
    /// written; R1 is silently skipped. This is valid — one-mate-only processing.
    #[test]
    fn test_pipe_exec_paired_r2_only() -> Result<()> {
        let r1 = write_fastx().call()?;
        let r2 = write_fastx().call()?;
        let cbq = NamedTempFile::with_suffix(".cbq")?;
        let encode_cmd = crate::cli::EncodeCommand::try_parse_from([
            "encode",
            r1.path().to_str().unwrap(),
            r2.path().to_str().unwrap(),
            "-o",
            cbq.path().to_str().unwrap(),
        ])?;
        crate::commands::encode::run(&encode_cmd)?;

        let fifo_dir = tempfile::tempdir()?;
        let basepath = fifo_dir.path().join("pipe").to_str().unwrap().to_string();
        let r2_out = NamedTempFile::with_suffix(".fastq")?;
        let r2_path = r2_out.path().to_str().unwrap().to_string();

        let cmd = crate::cli::PipeCommand::try_parse_from([
            "pipe",
            cbq.path().to_str().unwrap(),
            "-b",
            &basepath,
            "-p",
            "1",
            "-x",
            &format!("cat {{R2}} > {r2_path}"),
        ])?;
        super::run(&cmd)?;

        assert_eq!(
            count_fastx_records(r2_out.path())?,
            DEFAULT_NUM_RECORDS,
            "R2-only paired -x record count mismatch"
        );
        Ok(())
    }

    /// When a consumer exits non-zero, `run` returns an error — but the FIFOs must
    /// still be unlinked from disk by the `FifoGuard`, not leaked.
    #[test]
    fn test_pipe_fifos_cleaned_up_on_error() -> Result<()> {
        let fastq = write_fastx().call()?;
        let cbq = NamedTempFile::with_suffix(".cbq")?;
        encode(fastq.path(), cbq.path())?;

        let fifo_dir = tempfile::tempdir()?;
        let basepath = fifo_dir.path().join("pipe").to_str().unwrap().to_string();

        // Drain the FIFO (so writer threads complete), then exit non-zero so
        // `run` bails after the FIFOs have been created.
        let cmd = crate::cli::PipeCommand::try_parse_from([
            "pipe",
            cbq.path().to_str().unwrap(),
            "-b",
            &basepath,
            "-p",
            "2",
            "-x",
            "cat {} > /dev/null; exit 3",
        ])?;
        assert!(
            super::run(&cmd).is_err(),
            "run should propagate the non-zero consumer exit"
        );

        let leftover: Vec<_> = std::fs::read_dir(fifo_dir.path())?
            .filter_map(std::result::Result::ok)
            .map(|e| e.path())
            .collect();
        assert!(
            leftover.is_empty(),
            "FIFOs were leaked on error: {leftover:?}"
        );
        Ok(())
    }
}
