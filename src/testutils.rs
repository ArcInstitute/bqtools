use std::io::Write;
use std::path::Path;

use anyhow::Result;
use binseq::BinseqReader;
use bon::builder;
use niffler::Level;
use rand::{Rng, RngExt};
use tempfile::NamedTempFile;

use crate::cli::FileFormat;

pub const DEFAULT_NUM_RECORDS: usize = 100;
pub const DEFAULT_SEQ_LEN: usize = 100;

#[derive(Default, Clone, Copy, Debug)]
pub enum Compression {
    #[default]
    None,
    Gzip,
    Zstd,
}
impl Compression {
    pub fn all() -> impl Iterator<Item = Self> + Clone {
        [Self::None, Self::Gzip, Self::Zstd].into_iter()
    }

    pub fn suffix(self) -> &'static str {
        match self {
            Self::None => "",
            Self::Gzip => ".gz",
            Self::Zstd => ".zst",
        }
    }

    fn niffler_format(self) -> niffler::Format {
        match self {
            Self::None => niffler::Format::No,
            Self::Gzip => niffler::Format::Gzip,
            Self::Zstd => niffler::Format::Zstd,
        }
    }
}

fn write_fastq_record<W: Write>(wtr: &mut W, idx: usize, seq: &[u8]) -> Result<()> {
    let qual = vec![b'?'; seq.len()];
    writeln!(wtr, "@seq.{idx}")?;
    wtr.write_all(seq)?;
    writeln!(wtr, "\n+")?;
    wtr.write_all(&qual)?;
    writeln!(wtr)?;
    Ok(())
}

fn write_fasta_record<W: Write>(wtr: &mut W, idx: usize, seq: &[u8]) -> Result<()> {
    writeln!(wtr, ">seq.{idx}")?;
    wtr.write_all(seq)?;
    writeln!(wtr)?;
    Ok(())
}

fn random_sequence(rng: &mut impl Rng, slen: usize) -> Vec<u8> {
    (0..slen)
        .map(|_| match rng.random_range(0..=4) {
            0 => b'A',
            1 => b'C',
            2 => b'G',
            3 => b'T',
            _ => b'N',
        })
        .collect()
}

#[builder]
pub fn write_fastx(
    #[builder(default = FileFormat::Fastq)] format: FileFormat,
    #[builder(default)] comp: Compression,
    #[builder(default = DEFAULT_SEQ_LEN)] slen: usize,
    #[builder(default = DEFAULT_NUM_RECORDS)] nrec: usize,
) -> Result<NamedTempFile> {
    let suffix = format!("{}{}", format.fastx_suffix(), comp.suffix());
    let tmp = NamedTempFile::with_suffix(&suffix)?;
    let mut wtr = niffler::to_path(tmp.path(), comp.niffler_format(), Level::Three)?;
    let mut rng = rand::rng();
    for idx in 0..nrec {
        let seq = random_sequence(&mut rng, slen);
        match format {
            FileFormat::Fastq => write_fastq_record(&mut wtr, idx, &seq)?,
            FileFormat::Fasta => write_fasta_record(&mut wtr, idx, &seq)?,
            _ => unreachable!("write_fastx only supports Fastq and Fasta"),
        }
    }
    wtr.flush()?;
    Ok(tmp)
}

pub fn count_binseq(path: &Path) -> Result<usize> {
    let reader = BinseqReader::new(path.to_str().unwrap())?;
    Ok(reader.num_records()?)
}

/// Counts records in an uncompressed FASTA or FASTQ file, detected by extension.
pub fn count_fastx_records(path: &Path) -> Result<usize> {
    use std::io::{BufRead, BufReader};
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let file = std::fs::File::open(path)?;
    let reader = BufReader::new(file);
    let count = match ext {
        "fastq" | "fq" => reader.lines().count() / 4,
        "fasta" | "fa" => reader
            .lines()
            .filter(|l| l.as_ref().is_ok_and(|s| s.starts_with('>')))
            .count(),
        other => anyhow::bail!("count_fastx_records: unknown extension '.{other}'"),
    };
    Ok(count)
}
