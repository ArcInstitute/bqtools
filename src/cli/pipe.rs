use anyhow::Result;
use clap::Parser;

use crate::cli::FileFormat;

use super::InputBinseq;

/// Split BINSEQ files into multiple named pipes.
#[derive(Parser, Debug)]
pub struct PipeCommand {
    #[clap(flatten)]
    pub input: InputBinseq,

    #[clap(flatten)]
    pub pipe: PipeOptions,
}

#[derive(Parser, Debug)]
#[clap(next_help_heading = "PIPE OPTIONS")]
pub struct PipeOptions {
    #[clap(short = 'p', long, default_value = "0")]
    /// Number of pipes to make [0: as many as threads]
    num_pipes: usize,

    #[clap(short, long, help = "Output file format")]
    format: Option<FileFormat>,

    #[clap(
        short,
        long,
        help = "Base path for output files",
        default_value = "bqtools_fifo"
    )]
    basepath: String,

    /// Execute a shell command once per pipe, substituting FIFO paths.
    ///
    /// Use `{}` for the FIFO path (single-end), or `{R1}` / `{R2}` for the
    /// respective paths (paired-end). Referencing only one of `{R1}` / `{R2}`
    /// processes just that mate — the other channel's FIFOs are never created.
    /// `{n}` expands to the pipe index, useful for per-shard output paths.
    /// Mutually exclusive with `--exec-batch`.
    #[clap(short = 'x', long, conflicts_with = "exec_batch")]
    exec: Option<String>,

    /// Execute a single shell command with all FIFO paths substituted.
    ///
    /// `{}` (single-end) or `{R1}` / `{R2}` (paired-end) each expand to a
    /// space-joined list of every matching FIFO path. Writing `{R1} {R2}`
    /// adjacently interleaves the paths as pairs (r1_0 r2_0 r1_1 r2_1 …) so
    /// positional-argument tools receive each pair together.
    /// Mutually exclusive with `--exec`.
    #[clap(short = 'X', long, conflicts_with = "exec")]
    exec_batch: Option<String>,
}

impl PipeCommand {
    pub fn format(&self) -> Result<FileFormat> {
        let format = self.pipe.format.unwrap_or(FileFormat::Fastq);
        match format {
            FileFormat::Fasta | FileFormat::Fastq => Ok(format),
            _ => Err(anyhow::anyhow!("Unsupported output format")),
        }
    }
    pub fn num_pipes(&self) -> usize {
        match self.pipe.num_pipes {
            0 => num_cpus::get(),
            n => n.min(num_cpus::get()),
        }
    }
    pub fn basepath(&self) -> &str {
        &self.pipe.basepath
    }
    pub fn exec(&self) -> Option<&str> {
        self.pipe.exec.as_deref()
    }
    pub fn exec_batch(&self) -> Option<&str> {
        self.pipe.exec_batch.as_deref()
    }
}
