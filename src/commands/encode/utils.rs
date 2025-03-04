use std::io::Read;

use anyhow::{bail, Result};
use paraseq::fasta;
use paraseq::fasta::Reader as FastaReaderAlias;
use paraseq::fastq;
use paraseq::fastq::Reader as FastqReaderAlias;

type FastaReader = FastaReaderAlias<BoxReader>;
type FastqReader = FastqReaderAlias<BoxReader>;
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
