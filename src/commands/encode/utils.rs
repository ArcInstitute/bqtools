use std::io::Read;

use anyhow::{bail, Result};

use paraseq::{fasta, fastq, Record};

type FastaReader = fasta::Reader<BoxReader>;
type FastqReader = fastq::Reader<BoxReader>;
type BoxReader = Box<dyn Read + Send>;

pub fn get_sequence_len_fasta(reader: &mut FastaReader) -> Result<u32> {
    let mut rset = fasta::RecordSet::new(1);
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
    reader.reload(&mut rset);
    Ok(slen as u32)
}

pub fn get_interleaved_sequence_len_fasta(reader: &mut FastaReader) -> Result<(u32, u32)> {
    let mut rset = fasta::RecordSet::new(2);
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
    reader.reload(&mut rset);
    Ok((slen as u32, xlen as u32))
}

pub fn get_sequence_len_fastq(reader: &mut FastqReader) -> Result<u32> {
    let mut rset = fastq::RecordSet::new(1);
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
    reader.reload(&mut rset);
    Ok(slen as u32)
}

pub fn get_interleaved_sequence_len_fastq(reader: &mut FastqReader) -> Result<(u32, u32)> {
    let mut rset = fastq::RecordSet::new(2);
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
            bail!("Input file (interleaved) is missing R2 - cannot convert");
        };
        (r1.seq().len(), r2.seq().len())
    } else {
        bail!("Input file is empty - cannot convert");
    };
    reader.reload(&mut rset);
    Ok((slen as u32, xlen as u32))
}
