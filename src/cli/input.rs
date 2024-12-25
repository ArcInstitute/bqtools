use anyhow::{bail, Result};
use clap::{Parser, ValueEnum};
use std::io::Read;

use crate::commands::match_input;

#[derive(Parser, Debug)]
pub struct InputFile {
    #[clap(short, long, help = "Input file [default: stdin]")]
    pub input: Option<String>,

    #[clap(short, long, help = "Input file format")]
    pub format: Option<FileFormat>,
}
impl InputFile {
    pub fn format(&self) -> Result<FileFormat> {
        if let Some(format) = self.format {
            Ok(format)
        } else {
            if let Some(path) = self.input.as_ref() {
                if let Some(format) = FileFormat::from_path(path) {
                    Ok(format)
                } else {
                    bail!("Could not infer file format.")
                }
            } else {
                bail!("Could not infer file format.")
            }
        }
    }

    pub fn as_reader(&self) -> Result<Box<dyn Read>> {
        match_input(self.input.as_ref())
    }
}

#[derive(ValueEnum, Debug, Clone, Copy)]
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
