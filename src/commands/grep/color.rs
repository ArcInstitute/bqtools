use anyhow::Result;

use crate::cli::{FileFormat, Mate};

use std::{collections::HashSet, io::Write};

// ANSI color codes as byte constants
const RESET: &[u8] = b"\x1b[0m";
const RED_BOLD: &[u8] = b"\x1b[31;1m"; // Red + Bold

fn write_colored_sequence<W: Write>(
    writer: &mut W,
    buffer: &[u8],
    matches: &HashSet<(usize, usize)>,
) -> Result<()> {
    if matches.is_empty() {
        writer.write_all(buffer)?;
    } else {
        let mut seq_started = false;
        let mut last_end = 0;
        for (start, end) in matches.iter().copied() {
            // write the sequence without highlighting before the match
            if start > 0 && !seq_started {
                writer.write_all(&buffer[0..start])?;
                seq_started = true;
            }

            // write colored
            writer.write_all(RED_BOLD)?;
            writer.write_all(&buffer[start..end])?;
            writer.write_all(RESET)?;

            last_end = end;
        }

        // write the sequence without highlighting after the last match
        if last_end < buffer.len() {
            writer.write_all(&buffer[last_end..])?;
        }
    }
    Ok(())
}

fn write_colored_tsv<W: Write>(
    writer: &mut W,
    index: &[u8],
    buffer: &[u8],
    matches: &HashSet<(usize, usize)>,
) -> Result<()> {
    writer.write_all(index)?;
    writer.write_all(b"\t")?;
    write_colored_sequence(writer, buffer, matches)?;
    writer.write_all(b"\n")?;
    Ok(())
}

fn write_colored_fasta<W: Write>(
    writer: &mut W,
    index: &[u8],
    buffer: &[u8],
    matches: &HashSet<(usize, usize)>,
) -> Result<()> {
    writer.write_all(b">")?;
    writer.write_all(index)?;
    writer.write_all(b"\n")?;
    write_colored_sequence(writer, buffer, matches)?;
    writer.write_all(b"\n")?;
    Ok(())
}

fn write_colored_fastq<W: Write>(
    writer: &mut W,
    index: &[u8],
    buffer: &[u8],
    quality: &[u8],
    matches: &HashSet<(usize, usize)>,
) -> Result<()> {
    writer.write_all(b"@")?;
    writer.write_all(index)?;
    writer.write_all(b"\n")?;
    write_colored_sequence(writer, buffer, matches)?;
    writer.write_all(b"\n+\n")?;
    write_colored_sequence(writer, quality, matches)?;
    writer.write_all(b"\n")?;
    Ok(())
}

#[allow(clippy::match_wildcard_for_single_variants)]
fn write_colored_record<W: Write>(
    writer: &mut W,
    index: &[u8],
    sequence: &[u8],
    quality: &[u8],
    matches: &HashSet<(usize, usize)>,
    format: FileFormat,
) -> Result<()> {
    let qual_buf = &quality[..sequence.len()];
    match format {
        FileFormat::Tsv => write_colored_tsv(writer, index, sequence, matches),
        FileFormat::Fasta => write_colored_fasta(writer, index, sequence, matches),
        FileFormat::Fastq => write_colored_fastq(writer, index, sequence, qual_buf, matches),
        _ => unimplemented!("Colored output is not supported for {}", format.extension()),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn write_colored_record_pair<W: Write>(
    writer: &mut W,
    mate: Option<Mate>,
    sbuf: &[u8],
    squal: &[u8],
    sheader: &[u8],
    xbuf: &[u8],
    xqual: &[u8],
    xheader: &[u8],
    smatch: &HashSet<(usize, usize)>,
    xmatch: &HashSet<(usize, usize)>,
    format: FileFormat,
) -> Result<()> {
    match mate {
        Some(Mate::Both) => {
            write_colored_record(writer, sheader, sbuf, squal, smatch, format)?;
            write_colored_record(writer, xheader, xbuf, xqual, xmatch, format)?;
            Ok(())
        }
        Some(Mate::One) | None => {
            write_colored_record(writer, sheader, sbuf, squal, smatch, format)
        }
        Some(Mate::Two) => write_colored_record(writer, xheader, xbuf, xqual, xmatch, format),
    }
}
