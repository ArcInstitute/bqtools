mod fasta;
mod fastq;
mod processor;

pub use fasta::{encode_paired_fasta_parallel, encode_single_fasta_parallel};
pub use fastq::{encode_paired_fastq_parallel, encode_single_fastq_parallel};
use processor::Processor;
