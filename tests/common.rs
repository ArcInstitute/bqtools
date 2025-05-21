use std::io::Write;
use std::path::Path;

use anyhow::Result;
use bon::builder;
use niffler::Level;
use tempfile::NamedTempFile;

pub const COMMAND_NAME: &str = "bqtools";

#[derive(Default, Clone, Copy, Debug)]
pub enum CompressionStatus {
    #[default]
    Uncompressed,
    Gzip,
    Zstd,
}
impl CompressionStatus {
    pub fn enum_iter() -> impl Iterator<Item = Self> + Clone {
        let vals = [Self::Uncompressed, Self::Gzip, Self::Zstd];
        vals.into_iter()
    }

    pub fn suffix(&self) -> &str {
        match self {
            Self::Uncompressed => "",
            Self::Gzip => ".gz",
            Self::Zstd => ".zst",
        }
    }
}

#[derive(Default, Clone, Copy, Debug)]
pub enum FastxFormat {
    #[default]
    Fastq,
    Fasta,
}
impl FastxFormat {
    pub fn enum_iter() -> impl Iterator<Item = Self> + Clone {
        let vals = [Self::Fasta, Self::Fastq];
        vals.into_iter()
    }
    pub fn suffix(&self) -> &str {
        match self {
            Self::Fasta => ".fasta",
            Self::Fastq => ".fastq",
        }
    }
}

#[derive(Default, Clone, Copy, Debug)]
pub enum BinseqMode {
    #[default]
    Bq,
    Vbq,
}
impl BinseqMode {
    pub fn enum_iter() -> impl Iterator<Item = Self> + Clone {
        let vals = [Self::Bq, Self::Vbq];
        vals.into_iter()
    }
    pub fn suffix(&self) -> &str {
        match self {
            Self::Bq => ".bq",
            Self::Vbq => ".vbq",
        }
    }
}

fn write_fastq_to<W: Write>(wtr: &mut W, idx: usize, seq: &[u8], qual: &[u8]) -> Result<()> {
    writeln!(wtr, "@seq.{}", idx)?;
    wtr.write_all(seq)?;
    writeln!(wtr, "\n+")?;
    wtr.write_all(qual)?;
    write!(wtr, "\n")?;
    Ok(())
}

fn write_fasta_to<W: Write>(wtr: &mut W, idx: usize, seq: &[u8]) -> Result<()> {
    writeln!(wtr, ">seq.{}", idx)?;
    wtr.write_all(seq)?;
    write!(wtr, "\n")?;
    Ok(())
}

fn compression_passthrough(path: &Path, comp: CompressionStatus) -> Result<Box<dyn Write>> {
    match comp {
        CompressionStatus::Uncompressed => {
            Ok(niffler::to_path(path, niffler::Format::No, Level::Three)?)
        }
        CompressionStatus::Gzip => Ok(niffler::to_path(path, niffler::Format::Gzip, Level::Three)?),
        CompressionStatus::Zstd => Ok(niffler::to_path(path, niffler::Format::Zstd, Level::Three)?),
    }
}

fn with_suffix(format: FastxFormat, comp: CompressionStatus) -> String {
    format!("{}{}", format.suffix(), comp.suffix())
}

#[builder]
pub fn write_fastx(
    #[builder(default)] comp: CompressionStatus,
    #[builder(default)] format: FastxFormat,
    #[builder(default = 100)] slen: usize,
    #[builder(default = 100)] nrec: usize,
) -> Result<NamedTempFile> {
    let tempfile = NamedTempFile::with_suffix(with_suffix(format, comp))?;
    let mut handle = compression_passthrough(tempfile.path(), comp)?;
    let mut seqgen = nucgen::Sequence::with_capacity(slen);
    let mut rng = rand::rng();
    let qual = vec![b'?'; slen];
    for idx in 0..nrec {
        seqgen.clear_buffer();
        seqgen.fill_buffer(&mut rng, slen);
        let seq = seqgen.bytes();
        match format {
            FastxFormat::Fastq => write_fastq_to(&mut handle, idx, &seq, &qual)?,
            FastxFormat::Fasta => write_fasta_to(&mut handle, idx, &seq)?,
        }
    }
    handle.flush()?;

    Ok(tempfile)
}

pub fn output_tempfile(mode: BinseqMode) -> Result<NamedTempFile> {
    let tempfile = NamedTempFile::with_suffix(mode.suffix())?;
    Ok(tempfile)
}
