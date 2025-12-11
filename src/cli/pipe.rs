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
    #[clap(
        short = 'T',
        long,
        help = "Number of pipes to make / parallel BINSEQ readers",
        default_value = "0"
    )]
    threads: usize,

    #[clap(short, long, help = "Output file format")]
    format: Option<FileFormat>,

    #[clap(short, long, help = "Base path for output files")]
    basepath: String,
}

impl PipeCommand {
    pub fn format(&self) -> Result<FileFormat> {
        let format = self.pipe.format.unwrap_or(FileFormat::Fastq);
        match format {
            FileFormat::Fasta | FileFormat::Fastq => Ok(format),
            _ => Err(anyhow::anyhow!("Unsupported output format")),
        }
    }
    pub fn threads(&self) -> usize {
        match self.pipe.threads {
            0 => num_cpus::get(),
            n => n.min(num_cpus::get()),
        }
    }
    pub fn basepath(&self) -> &str {
        &self.pipe.basepath
    }
}
