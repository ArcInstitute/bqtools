use anyhow::{bail, Result};
use clap::{Parser, ValueEnum};
use std::io::Read;

use crate::commands::match_input;

#[derive(Parser, Debug)]
pub struct InputFile {
    #[clap(help = "Input file [default: stdin]", num_args = 0..=2)]
    pub input: Vec<String>,

    #[clap(short, long, help = "Input file format")]
    pub format: Option<FileFormat>,
}
impl InputFile {
    pub fn format(&self) -> Result<FileFormat> {
        if let Some(format) = self.format {
            Ok(format)
        } else {
            match self.input.len() {
                0 => bail!("Can not infer file format from stdin."),
                _ => {
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
        }
    }

    pub fn as_reader(&self) -> Result<Box<dyn Read>> {
        match self.input.len() {
            0 => match_input(None),
            1 => match_input(Some(&self.input[0])),
            _ => bail!("Multiple input files are not supported."),
        }
    }

    pub fn as_reader_pair(&self) -> Result<(Box<dyn Read>, Box<dyn Read>)> {
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

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileFormat {
    Fasta,
    Fastq,
}
impl FileFormat {
    pub fn from_path(path: &str) -> Option<Self> {
        let ext = match path.split('.').last()? {
            "gz" => path.split('.').nth_back(1)?,
            ext => ext,
        };
        match ext {
            "fasta" | "fa" => Some(Self::Fasta),
            "fastq" | "fq" => Some(Self::Fastq),
            _ => None,
        }
    }
}

#[derive(Parser, Debug)]
pub struct InputFastq {
    #[clap(short = 'i', long, help = "Input FASTQ file [default: stdin]")]
    pub input: Option<String>,
}
impl InputFastq {
    pub fn as_reader(&self) -> Result<Box<dyn Read>> {
        match_input(self.input.as_ref())
    }
}

#[derive(Parser, Debug)]
pub struct InputFasta {
    #[clap(short = 'i', long, help = "Input FASTA file [default: stdin]")]
    pub input: Option<String>,
}
impl InputFasta {
    pub fn as_reader(&self) -> Result<Box<dyn Read>> {
        match_input(self.input.as_ref())
    }
}

#[derive(Parser, Debug)]
pub struct InputBinseq {
    #[clap(short = 'i', long, help = "Input binseq file [default: stdin]")]
    pub input: Option<String>,
}
impl InputBinseq {
    pub fn as_reader(&self) -> Result<Box<dyn Read>> {
        match_input(self.input.as_ref())
    }
}
