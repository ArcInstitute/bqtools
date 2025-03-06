use anyhow::{bail, Result};
use binseq::Policy;
use clap::{Parser, ValueEnum};
use std::io::Write;
use vbinseq::Policy as VPolicy;

use crate::{
    cli::FileFormat,
    commands::{compress_gzip_passthrough, compress_zstd_passthrough, match_output},
};

#[derive(Parser, Debug)]
#[clap(next_help_heading = "OUTPUT FILE OPTIONS")]
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

    /// Designate which of the two mates is being processed
    ///
    /// This is only relevant for paired BINSEQ files. The mate number is 1-based.
    #[clap(short = 'm', long, default_value = "both")]
    pub mate: Mate,

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
    pub fn as_writer(&self) -> Result<Box<dyn Write + Send>> {
        let writer = match_output(self.output.as_ref())?;
        compress_gzip_passthrough(writer, self.compress(), self.threads())
    }

    pub fn compress(&self) -> bool {
        self.output.as_ref().map_or(self.compress, |path| {
            if path.ends_with(".gz") {
                true
            } else {
                self.compress
            }
        })
    }

    pub fn mate(&self) -> Mate {
        self.mate
    }

    pub fn format(&self) -> Result<FileFormat> {
        if let Some(format) = self.format {
            Ok(format)
        } else {
            if let Some(path) = self.output.as_ref() {
                if let Some(format) = FileFormat::from_path(path) {
                    return Ok(format);
                }
                anyhow::bail!("Could not infer file format.")
            }
            Ok(FileFormat::Tsv)
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

    pub fn as_paired_writer(
        &self,
        format: FileFormat,
    ) -> Result<(Box<dyn Write + Send>, Box<dyn Write + Send>)> {
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

#[derive(ValueEnum, PartialEq, Eq, Clone, Copy, Debug, Default)]
pub enum Mate {
    #[clap(name = "1")]
    One,
    #[clap(name = "2")]
    Two,
    #[default]
    Both,
}

#[derive(Parser, Debug)]
#[clap(next_help_heading = "OUTPUT BINSEQ OPTIONS")]
pub struct OutputBinseq {
    #[clap(
        short = 'o',
        long,
        help = "Output binseq file [default: stdout]",
        required_unless_present = "pipe"
    )]
    pub output: Option<String>,

    /// Defines the BINSEQ mode to use.
    #[clap(short = 'm', long)]
    pub mode: Option<BinseqMode>,

    /// Policy for handling Ns in sequences
    #[clap(short = 'p', long, default_value = "r")]
    pub policy: PolicyWrapper,

    /// Skip ZSTD compression of VBQ blocks (default: compressed)
    ///
    /// Only used by vbq.
    #[clap(short = 'u', long)]
    pub uncompressed: bool,

    /// Skip inclusion of quality scores (default: included)
    ///
    /// Only used by vbq.
    #[clap(short = 'Q', long)]
    pub skip_quality: bool,

    /// VBQ virtual block size (in bytes)
    ///
    /// Only used by vbq
    #[clap(short = 'B', long, value_parser = parse_memory_size, default_value = "128K")]
    pub block_size: usize,

    /// Number of threads to use for parallel reading and writing.
    ///
    /// The number of threads is by default 1, 0 sets to maximum, and all other values are clamped to maximum.
    #[clap(short = 'T', long, default_value = "1")]
    pub threads: usize,

    /// Zstd compression level
    /// The compression level is between 1 and 22, with 3 being the default.
    /// Higher levels provide better compression at the cost of speed.
    /// Level 0 disables compression.
    #[clap(short, long, default_value = "3")]
    pub level: i32,

    /// Pipe the output to stdout
    #[clap(short = 'P', long)]
    pub pipe: bool,
}
impl OutputBinseq {
    pub fn as_writer(&self) -> Result<Box<dyn Write + Send>> {
        let writer = match_output(self.output.as_ref())?;
        compress_zstd_passthrough(writer, self.compress(), self.level(), self.threads())
    }

    pub fn owned_path(&self) -> Option<String> {
        self.output.clone()
    }

    pub fn mode(&self) -> Result<BinseqMode> {
        if let Some(mode) = self.mode {
            Ok(mode)
        } else if let Some(ref path) = self.output {
            BinseqMode::determine(path)
        } else {
            // STDOUT
            Ok(BinseqMode::default())
        }
    }

    pub fn compress(&self) -> bool {
        !self.uncompressed
    }

    pub fn quality(&self) -> bool {
        !self.skip_quality
    }

    pub fn threads(&self) -> usize {
        match self.threads {
            0 => num_cpus::get(),
            n => n.min(num_cpus::get()),
        }
    }

    fn level(&self) -> i32 {
        self.level.clamp(0, 22)
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum PolicyWrapper {
    /// Ignore any sequence if it contains an N
    #[clap(name = "i")]
    IgnoreSequence,

    /// Panic if any sequence contains an N
    #[clap(name = "p")]
    BreakOnInvalid,

    /// Randomly draw a nucleotide for each N in sequences.
    #[clap(name = "r")]
    RandomDraw,

    /// Sets all Ns to A
    #[clap(name = "a")]
    SetToA,

    /// Sets all Ns to C
    #[clap(name = "c")]
    SetToC,

    /// Sets all Ns to G
    #[clap(name = "g")]
    SetToG,

    /// Sets all Ns to T
    #[clap(name = "t")]
    SetToT,
}
impl From<PolicyWrapper> for Policy {
    fn from(value: PolicyWrapper) -> Self {
        match value {
            PolicyWrapper::IgnoreSequence => Policy::IgnoreSequence,
            PolicyWrapper::BreakOnInvalid => Policy::BreakOnInvalid,
            PolicyWrapper::RandomDraw => Policy::RandomDraw,
            PolicyWrapper::SetToA => Policy::SetToA,
            PolicyWrapper::SetToC => Policy::SetToC,
            PolicyWrapper::SetToG => Policy::SetToG,
            PolicyWrapper::SetToT => Policy::SetToT,
        }
    }
}
impl From<PolicyWrapper> for VPolicy {
    fn from(value: PolicyWrapper) -> Self {
        match value {
            PolicyWrapper::IgnoreSequence => VPolicy::IgnoreSequence,
            PolicyWrapper::BreakOnInvalid => VPolicy::BreakOnInvalid,
            PolicyWrapper::RandomDraw => VPolicy::RandomDraw,
            PolicyWrapper::SetToA => VPolicy::SetToA,
            PolicyWrapper::SetToC => VPolicy::SetToC,
            PolicyWrapper::SetToG => VPolicy::SetToG,
            PolicyWrapper::SetToT => VPolicy::SetToT,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum, Default)]
pub enum BinseqMode {
    #[clap(name = "bq")]
    #[default]
    Binseq,
    #[clap(name = "vbq")]
    VBinseq,
}
impl BinseqMode {
    pub fn determine(path: &str) -> Result<Self> {
        if path.ends_with(".bq") {
            Ok(Self::Binseq)
        } else if path.ends_with(".vbq") {
            Ok(Self::VBinseq)
        } else {
            bail!("Could not determine BINSEQ output mode from path: {path}");
        }
    }
}

fn parse_memory_size(input: &str) -> Result<usize, String> {
    let input = input.trim().to_uppercase();
    let last_char = input.chars().last().unwrap_or('0');

    let (number_str, multiplier) = match last_char {
        'K' | 'k' => (&input[..input.len() - 1], 1024),
        'M' | 'm' => (&input[..input.len() - 1], 1024 * 1024),
        'G' | 'g' => (&input[..input.len() - 1], 1024 * 1024 * 1024),
        _ if last_char.is_ascii_digit() => (input.as_str(), 1),
        _ => return Err(format!("Invalid memory size format: {}", input)),
    };

    match number_str.parse::<usize>() {
        Ok(number) => Ok(number * multiplier),
        Err(_) => Err(format!("Failed to parse number: {}", number_str)),
    }
}
