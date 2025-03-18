use anyhow::{bail, Result};

use crate::cli::{BinseqMode, IndexCommand};

pub fn index_path(path: &str, verbose: bool) -> Result<()> {
    let reader = vbinseq::MmapReader::new(path)?;
    if reader.index_path().exists() {
        std::fs::remove_file(reader.index_path())?;
    }
    let index = reader.load_index()?;

    if verbose {
        let n_blocks = index.n_blocks();
        let mut total_records = 0;
        index.ranges().iter().for_each(|r| {
            total_records += r.block_records;
        });
        let records_per_block = total_records as f64 / n_blocks as f64;

        println!("Index path: {}", reader.index_path().display());
        println!("Number of blocks: {}", n_blocks);
        println!("Number of records: {}", total_records);
        println!("Average records per block: {:.2}", records_per_block);
    }

    Ok(())
}

pub fn run(args: IndexCommand) -> Result<()> {
    if let BinseqMode::Binseq = args.input.mode()? {
        bail!(
            "Only VBINSEQ files are indexable - {} is a BINSEQ file",
            args.input.path()
        )
    }
    index_path(args.input.path(), args.verbose)
}
