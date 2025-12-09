use anyhow::Result;
use binseq::prelude::*;

use crate::cli::{FileFormat, Mate};

use std::{collections::HashSet, io::Write};

// ANSI color codes as byte constants
const RESET: &[u8] = b"\x1b[0m";
const RED_BOLD: &[u8] = b"\x1b[31;1m"; // Red + Bold
type Interval = (usize, usize);

fn overlap(iv: Interval, jv: Interval) -> bool {
    iv.0 <= jv.1 && jv.0 <= iv.1
}

fn load_and_merge_matches(matches: &HashSet<Interval>, interval_buffer: &mut Vec<Interval>) {
    // clear the buffer
    interval_buffer.clear();

    // load matches into the buffer
    interval_buffer.extend(matches.iter().copied());

    // sort the intervals
    interval_buffer.sort_unstable();

    if interval_buffer.len() <= 1 {
        return;
    }
    let mut write_idx = 0;
    let mut current = interval_buffer[0];

    for i in 1..interval_buffer.len() {
        let iv = interval_buffer[i];
        if overlap(current, iv) {
            current = (current.0.min(iv.0), current.1.max(iv.1));
        } else {
            interval_buffer[write_idx] = current;
            write_idx += 1;
            current = iv;
        }
    }
    interval_buffer[write_idx] = current;
    interval_buffer.truncate(write_idx + 1);
}

fn write_colored_sequence<W: Write>(
    writer: &mut W,
    buffer: &[u8],
    matches: &HashSet<Interval>,
    interval_buffer: &mut Vec<Interval>,
) -> Result<()> {
    if matches.is_empty() {
        writer.write_all(buffer)?;
    } else {
        load_and_merge_matches(matches, interval_buffer);
        let mut pos = 0; // Track current position in buffer
        for (start, end) in interval_buffer.iter().copied() {
            // Write uncolored region from last position to this match
            if start > pos {
                writer.write_all(&buffer[pos..start])?;
            }

            // Write colored match
            writer.write_all(RED_BOLD)?;
            writer.write_all(&buffer[start..end])?;
            writer.write_all(RESET)?;

            pos = end; // Update position to end of this match
        }

        // Write remaining uncolored region after last match
        if pos < buffer.len() {
            writer.write_all(&buffer[pos..])?;
        }
    }
    Ok(())
}

fn write_colored_tsv<W: Write>(
    writer: &mut W,
    index: &[u8],
    buffer: &[u8],
    matches: &HashSet<Interval>,
    interval_buffer: &mut Vec<Interval>,
) -> Result<()> {
    writer.write_all(index)?;
    writer.write_all(b"\t")?;
    write_colored_sequence(writer, buffer, matches, interval_buffer)?;
    writer.write_all(b"\n")?;
    Ok(())
}

fn write_colored_fasta<W: Write>(
    writer: &mut W,
    index: &[u8],
    buffer: &[u8],
    matches: &HashSet<Interval>,
    interval_buffer: &mut Vec<Interval>,
) -> Result<()> {
    writer.write_all(b">")?;
    writer.write_all(index)?;
    writer.write_all(b"\n")?;
    write_colored_sequence(writer, buffer, matches, interval_buffer)?;
    writer.write_all(b"\n")?;
    Ok(())
}

fn write_colored_fastq<W: Write>(
    writer: &mut W,
    index: &[u8],
    buffer: &[u8],
    quality: &[u8],
    matches: &HashSet<Interval>,
    interval_buffer: &mut Vec<Interval>,
) -> Result<()> {
    writer.write_all(b"@")?;
    writer.write_all(index)?;
    writer.write_all(b"\n")?;
    write_colored_sequence(writer, buffer, matches, interval_buffer)?;
    writer.write_all(b"\n+\n")?;
    write_colored_sequence(writer, quality, matches, interval_buffer)?;
    writer.write_all(b"\n")?;
    Ok(())
}

#[allow(clippy::match_wildcard_for_single_variants)]
fn write_colored_record<W: Write>(
    writer: &mut W,
    index: &[u8],
    sequence: &[u8],
    quality: &[u8],
    matches: &HashSet<Interval>,
    format: FileFormat,
    interval_buffer: &mut Vec<Interval>,
) -> Result<()> {
    let qual_buf = &quality[..sequence.len()];
    match format {
        FileFormat::Tsv => write_colored_tsv(writer, index, sequence, matches, interval_buffer),
        FileFormat::Fasta => write_colored_fasta(writer, index, sequence, matches, interval_buffer),
        FileFormat::Fastq => {
            write_colored_fastq(writer, index, sequence, qual_buf, matches, interval_buffer)
        }
        _ => unimplemented!("Colored output is not supported for {}", format.extension()),
    }
}

pub fn write_colored_record_pair<W: Write>(
    writer: &mut W,
    mate: Option<Mate>,
    ctx: &Ctx,
    smatch: &HashSet<Interval>,
    xmatch: &HashSet<Interval>,
    format: FileFormat,
    interval_buffer: &mut Vec<Interval>,
) -> Result<()> {
    match mate {
        Some(Mate::Both) => {
            write_colored_record(
                writer,
                ctx.sheader(),
                ctx.sbuf(),
                ctx.squal(),
                smatch,
                format,
                interval_buffer,
            )?;
            write_colored_record(
                writer,
                ctx.xheader(),
                ctx.xbuf(),
                ctx.xqual(),
                xmatch,
                format,
                interval_buffer,
            )?;
            Ok(())
        }
        Some(Mate::One) | None => write_colored_record(
            writer,
            ctx.sheader(),
            ctx.sbuf(),
            ctx.squal(),
            smatch,
            format,
            interval_buffer,
        ),
        Some(Mate::Two) => write_colored_record(
            writer,
            ctx.xheader(),
            ctx.xbuf(),
            ctx.xqual(),
            xmatch,
            format,
            interval_buffer,
        ),
    }
}
