use std::io::Read;

use anyhow::{bail, Result};
use paraseq::{
    fastx,
    rust_htslib::{self, bam::Read as BamRead},
    Record,
};

type BoxReader = Box<dyn Read + Send>;

pub fn get_sequence_len(reader: &mut fastx::Reader<BoxReader>) -> Result<u32> {
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
    Ok(slen as u32)
}

pub fn get_sequence_len_htslib(path: &str, paired: bool) -> Result<(u32, u32)> {
    let mut reader = rust_htslib::bam::Reader::from_path(path)?;
    let mut slen = 0;
    let mut xlen = 0;

    let mut rc_records = reader.rc_records().into_iter();

    if let Some(res) = rc_records.next() {
        let rec = res?;
        slen = rec.seq_len()
    }

    if paired {
        if let Some(res) = rc_records.next() {
            let rec = res?;
            xlen = rec.seq_len()
        }
    }
    Ok((slen as u32, xlen as u32))
}

pub fn get_interleaved_sequence_len(reader: &mut fastx::Reader<BoxReader>) -> Result<(u32, u32)> {
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
    Ok((slen as u32, xlen as u32))
}
