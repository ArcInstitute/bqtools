mod fastq;
mod processor;

pub use fastq::{encode_paired_fastq_parallel, encode_single_fastq_parallel};
use processor::Processor;
