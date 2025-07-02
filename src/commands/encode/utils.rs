use std::{io::Read, ops::Div};

use anyhow::{bail, Result};
use binseq::bq::BinseqHeader;
use paraseq::{fastx, Record};

use crate::cli::{PadMode, TruncateConfig, TruncateMate, TruncateMode};

type BoxReader = Box<dyn Read + Send>;

/// Default padding character for the processor.
const PAD_CHAR: u8 = b'N';
/// Preinitialized padding stack for the processor.
const PAD_STACK: [u8; 256] = [PAD_CHAR; 256];

pub fn get_sequence_len(
    reader: &mut fastx::Reader<BoxReader>,
    truncate_conf: Option<TruncateConfig>,
    primary: bool,
) -> Result<u32> {
    let mut rset = reader.new_record_set_with_size(1);
    let slen = if rset.fill(reader)? {
        let record = if let Some(record) = rset.iter().next() {
            record?
        } else {
            bail!("Input file is empty - cannot convert");
        };
        record.seq().len()
    } else {
        bail!("Input file is empty - cannot convert");
    };
    reader.reload(&mut rset)?;

    if let Some(conf) = truncate_conf {
        match conf.mate {
            TruncateMate::Both => Ok(conf.mode.inner() as u32),
            TruncateMate::Primary => {
                if primary {
                    Ok(conf.mode.inner() as u32)
                } else {
                    Ok(slen as u32)
                }
            }
            TruncateMate::Extended => {
                if primary {
                    Ok(slen as u32)
                } else {
                    Ok(conf.mode.inner() as u32)
                }
            }
        }
    } else {
        Ok(slen as u32)
    }
}

pub fn get_interleaved_sequence_len(
    reader: &mut fastx::Reader<BoxReader>,
    truncate: Option<TruncateConfig>,
) -> Result<(u32, u32)> {
    let mut rset = reader.new_record_set_with_size(2);
    let (slen, xlen) = if rset.fill(reader)? {
        let mut rset_iter = rset.iter();
        let r1 = if let Some(record) = rset_iter.next() {
            record?
        } else {
            bail!("Input file is empty - cannot convert");
        };
        let r2 = if let Some(record) = rset_iter.next() {
            record?
        } else {
            bail!("Input file is empty - cannot convert");
        };
        (r1.seq().len(), r2.seq().len())
    } else {
        bail!("Input file (interleaved) is missing R2 - cannot convert");
    };
    reader.reload(&mut rset)?;

    if let Some(conf) = truncate {
        match conf.mate {
            TruncateMate::Both => Ok((conf.mode.inner() as u32, conf.mode.inner() as u32)),
            TruncateMate::Primary => Ok((conf.mode.inner() as u32, xlen as u32)),
            TruncateMate::Extended => Ok((slen as u32, conf.mode.inner() as u32)),
        }
    } else {
        Ok((slen as u32, xlen as u32))
    }
}

/// Truncate the sequence based on the provided configuration (if necessary)
pub fn truncate_sequence(seq: &[u8], primary: bool, conf: Option<TruncateConfig>) -> &[u8] {
    if let Some(conf) = conf {
        match (conf.mate, primary) {
            (TruncateMate::Both | TruncateMate::Primary, true) => match conf.mode {
                TruncateMode::Prefix(size) => &seq[..size.min(seq.len())],
                TruncateMode::Suffix(size) => &seq[seq.len().saturating_sub(size)..],
            },
            (TruncateMate::Both | TruncateMate::Extended, false) => match conf.mode {
                TruncateMode::Prefix(size) => &seq[..size.min(seq.len())],
                TruncateMode::Suffix(size) => &seq[seq.len().saturating_sub(size)..],
            },
            _ => seq,
        }
    } else {
        seq
    }
}

/// Pad the sequence based on the provided configuration (if necessary)
///
/// Will transfer to sequence and padding to an intermediate buffer.
///
/// Will *always* clear the buffer on call.
pub fn pad_sequence<'a>(
    pad: &'a mut Vec<u8>,
    seq: &'a [u8],
    primary: bool,
    padmode: Option<PadMode>,
    header: BinseqHeader,
) {
    // Clear the padding vector
    pad.clear();

    let Some(mode) = padmode else { return };

    let comp_size = if primary {
        header.slen as usize
    } else {
        header.xlen as usize
    };

    // Define a closure for padding
    let add_to_pad = |pad: &mut Vec<u8>, n: usize| {
        for _ in 0..n.div(PAD_STACK.len()) {
            pad.extend_from_slice(&PAD_STACK);
        }
        pad.extend_from_slice(&PAD_STACK[..n % PAD_STACK.len()]);
    };

    match comp_size.saturating_sub(seq.len()) {
        n if n > 0 => {
            // Handle padding based on prefix status
            match mode {
                PadMode::Prefix => {
                    add_to_pad(pad, n);
                    pad.extend_from_slice(&seq);
                }
                PadMode::Suffix => {
                    pad.extend_from_slice(&seq);
                    add_to_pad(pad, n);
                }
            }
        }
        _ => {
            // No padding needed
        }
    }
}
