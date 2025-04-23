use anyhow::Result;
use binseq::bq::MmapReader;

use crate::cli::CountCommand;

fn log_reader(reader: &MmapReader) {
    let header = reader.header();
    let num_records = reader.num_records();
    println!("Format Version    : {}", header.format);
    println!("Sequence Length   : {}", header.slen);
    if header.xlen > 0 {
        println!("Extended Length   : {}", header.xlen);
    }
    println!("Number of records : {num_records}");
}

pub fn run(args: &CountCommand) -> Result<()> {
    let reader = MmapReader::new(args.input.path())?;
    log_reader(&reader);
    Ok(())
}
