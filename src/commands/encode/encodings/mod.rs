mod fasta;
mod fastq;

pub use fasta::{encode_paired_fasta, encode_single_fasta};
pub use fastq::{encode_paired_fastq, encode_single_fastq};
