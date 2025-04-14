use anyhow::Result;
use clap::Parser;

use super::{InputBinseq, OutputFile};

/// Subsample a BINSEQ file and output to FASTQ, FASTA, or TSV
#[derive(Parser)]
pub struct SampleCommand {
    #[clap(flatten)]
    pub input: InputBinseq,

    #[clap(flatten)]
    pub output: OutputFile,

    #[clap(flatten)]
    pub sample: SampleArgs,
}

#[derive(Parser, Debug)]
#[clap(next_help_heading = "SAMPLE OPTIONS")]
pub struct SampleArgs {
    /// Fraction of the input reads to sample
    #[clap(short = 'F', long)]
    pub fraction: f64,

    /// Seed to use for random sampling
    #[clap(short = 'S', long, default_value = "42")]
    pub seed: u64,
}
impl SampleArgs {
    pub fn validate(&self) -> Result<()> {
        if self.fraction <= 0.0 || self.fraction > 1.0 {
            anyhow::bail!("Fraction must be between 0 and 1");
        }
        Ok(())
    }
}
