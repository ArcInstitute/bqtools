use anyhow::{bail, Result};
use clap::Parser;
use std::io::Read;

use super::{BinseqMode, FileFormat};
use crate::commands::match_input;

#[derive(Parser, Debug, Clone)]
#[clap(next_help_heading = "INPUT FILE OPTIONS")]
pub struct InputFile {
    /// Input file [default: stdin]
    ///
    /// Can specify either zero (stdin), one, or two (paired) input files.
    #[clap(help = "Input file [default: stdin]", num_args = 0..=2)]
    pub input: Vec<String>,

    #[clap(short, long, help = "Input file format")]
    pub format: Option<FileFormat>,

    #[clap(short = 'I', long, help = "Interleaved input file format")]
    pub interleaved: bool,
}
impl InputFile {
    pub fn format(&self) -> Result<FileFormat> {
        if let Some(format) = self.format {
            Ok(format)
        } else {
            if self.input.is_empty() {
                bail!("Can not infer file format from stdin.");
            }
            // Identify file format for each input file
            let formats = self
                .input
                .iter()
                .map(|path| {
                    // Infer file format for each input file
                    if let Some(format) = FileFormat::from_path(path) {
                        Ok(format)

                    // Bail if file format could not be inferred
                    } else {
                        bail!("Could not infer file format.")
                    }
                })
                .collect::<Result<Vec<_>>>()?;

            // Check if all formats are the same
            if formats.iter().all(|&f| f == formats[0]) {
                // Return the format
                Ok(formats[0])
            } else {
                // Bail if formats are inconsistent
                bail!("Inconsistent file formats.")
            }
        }
    }

    pub fn as_reader(&self) -> Result<Box<dyn Read + Send>> {
        match self.input.len() {
            0 => match_input(None),
            1 => match_input(Some(&self.input[0])),
            _ => bail!("Multiple input files are not supported."),
        }
    }

    pub fn as_reader_pair(&self) -> Result<(Box<dyn Read + Send>, Box<dyn Read + Send>)> {
        match self.input.len() {
            2 => Ok((
                match_input(Some(&self.input[0]))?,
                match_input(Some(&self.input[1]))?,
            )),
            _ => bail!("Two input files are required."),
        }
    }

    pub fn paired(&self) -> bool {
        self.input.len() == 2
    }
}

#[derive(Parser, Debug)]
#[clap(next_help_heading = "INPUT FILE OPTIONS")]
pub struct InputBinseq {
    #[clap(help = "Input binseq file")]
    pub input: String,
}
impl InputBinseq {
    pub fn path(&self) -> &str {
        &self.input
    }

    pub fn mode(&self) -> Result<BinseqMode> {
        BinseqMode::determine(self.path())
    }
}

#[derive(Parser, Debug)]
#[clap(next_help_heading = "INPUT FILE OPTIONS")]
pub struct MultiInputBinseq {
    /// Input binseq files
    #[clap(num_args = 1..)]
    pub input: Vec<String>,
}
