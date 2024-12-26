use anyhow::Result;
use clap::Parser;
use std::io::Write;

use crate::{
    cli::FileFormat,
    commands::{compress_gzip_passthrough, compress_zstd_passthrough, match_output},
};

#[derive(Parser, Debug)]
pub struct OutputFile {
    #[clap(short = 'o', long, help = "Output file [default: stdout]")]
    pub output: Option<String>,

    #[clap(
        short,
        long,
        help = "Output file format prefix (required for paired BINSEQ files",
        conflicts_with = "output"
    )]
    pub prefix: Option<String>,

    #[clap(short, long, help = "Output file format")]
    pub format: Option<FileFormat>,

    #[clap(
        short,
        long,
        help = "Gzip compress output file",
        default_value = "false"
    )]
    pub compress: bool,

    #[clap(
        short = 'T',
        long,
        help = "Number of threads to use for parallel compression (0 for auto)",
        default_value = "1"
    )]
    pub threads: usize,
}
impl OutputFile {
    pub fn as_writer(&self) -> Result<Box<dyn Write>> {
        let writer = match_output(self.output.as_ref())?;
        compress_gzip_passthrough(writer, self.compress, self.threads())
    }

    pub fn format(&self) -> Result<FileFormat> {
        if let Some(format) = self.format {
            Ok(format)
        } else {
            if let Some(path) = self.output.as_ref() {
                if let Some(format) = FileFormat::from_path(path) {
                    Ok(format)
                } else {
                    anyhow::bail!("Could not infer file format.")
                }
            } else {
                anyhow::bail!("Can not infer file format from stdout.")
            }
        }
    }

    /// Returns the number of threads to use for parallel compression
    ///
    /// The number of threads is by default 1, 0 sets to maximum, and all other values are clamped to maximum.
    pub fn threads(&self) -> usize {
        match self.threads {
            0 => num_cpus::get(),
            n => n.min(num_cpus::get()),
        }
    }

    pub fn as_paired_writer(&self, format: FileFormat) -> Result<(Box<dyn Write>, Box<dyn Write>)> {
        // Check for prefix
        let prefix = self.prefix.as_ref().ok_or_else(|| {
            anyhow::anyhow!("Output file format prefix is required for paired BINSEQ files")
        })?;

        // Construct the output file names
        let r1_name = if self.compress {
            format!("{}_R1.{}.gz", prefix, format.extension())
        } else {
            format!("{}_R1.{}", prefix, format.extension())
        };
        let r2_name = if self.compress {
            format!("{}_R2.{}.gz", prefix, format.extension())
        } else {
            format!("{}_R2.{}", prefix, format.extension())
        };

        // Open the output files
        let r1 = match_output(Some(&r1_name))?;
        let r2 = match_output(Some(&r2_name))?;

        // Compress the output files (if necessary)
        let r1 = compress_gzip_passthrough(r1, self.compress, self.threads())?;
        let r2 = compress_gzip_passthrough(r2, self.compress, self.threads())?;

        Ok((r1, r2))
    }
}

#[derive(Parser, Debug)]
pub struct OutputBinseq {
    #[clap(short = 'o', long, help = "Output binseq file [default: stdout]")]
    pub output: Option<String>,

    /// Zstd compress output file
    #[clap(short, long)]
    pub compress: bool,

    /// Number of threads to use for parallel compression
    /// The number of threads is by default 1, 0 sets to maximum, and all other values are clamped to maximum.
    #[clap(short = 'T', long, default_value = "1")]
    pub threads: usize,

    /// Zstd compression level
    /// The compression level is between 1 and 22, with 3 being the default.
    /// Higher levels provide better compression at the cost of speed.
    /// Level 0 disables compression.
    #[clap(short, long, default_value = "3")]
    pub level: i32,
}
impl OutputBinseq {
    pub fn as_writer(&self) -> Result<Box<dyn Write + Send>> {
        let writer = match_output(self.output.as_ref())?;
        compress_zstd_passthrough(writer, self.compress(), self.level(), self.threads())
    }

    fn compress(&self) -> bool {
        self.output.as_ref().map_or(self.compress, |path| {
            if path.ends_with(".bqz") {
                true
            } else {
                self.compress
            }
        })
    }

    fn threads(&self) -> usize {
        match self.threads {
            0 => num_cpus::get(),
            n => n.min(num_cpus::get()),
        }
    }

    fn level(&self) -> i32 {
        self.level.min(22).max(0)
    }
}
