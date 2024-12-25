mod process;

use anyhow::Result;
use std::io::Read;

use crate::{
    cli::{EncodeCommand, FileFormat},
    process_single_fastx,
};

pub fn run(args: EncodeCommand) -> Result<()> {
    // Open the IO handles
    let in_handle = args.input.as_reader()?;
    let out_handle = args.output.as_writer()?;

    // Compression passthrough on input
    let (in_handle, _comp) = niffler::get_reader(in_handle)?;

    match args.input.format()? {
        FileFormat::Fastq => {
            process_single_fastx!(seq_io::fastq::Reader<Box<dyn Read>>, in_handle, out_handle)
        }
        FileFormat::Fasta => {
            process_single_fastx!(seq_io::fasta::Reader<Box<dyn Read>>, in_handle, out_handle)
        }
    }
}
