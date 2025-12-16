use anyhow::{bail, Result};
use binseq::{bq, vbq, BitSize, Policy};
use paraseq::{
    fastx::{self, Format},
    prelude::{PairedParallelProcessor, ParallelProcessor},
};

use crate::{
    cli::{BinseqMode, OutputBinseq},
    commands::{
        encode::{
            processor::{BinseqProcessor, VBinseqProcessor},
            utils::{get_interleaved_sequence_len, get_sequence_len},
        },
        match_output,
    },
    types::{BoxedReader, BoxedWriter},
};

pub struct Config {
    compress: bool,
    quality: bool,
    block_size: usize,
    policy: Policy,
    bitsize: BitSize,
    headers: bool,
    threads: usize,
}
impl From<OutputBinseq> for Config {
    fn from(output: OutputBinseq) -> Self {
        Config {
            compress: output.compress(),
            quality: output.quality(),
            block_size: output.block_size(),
            policy: output.policy.into(),
            bitsize: output.bitsize(),
            headers: output.headers,
            threads: output.threads(),
        }
    }
}

pub fn encode_collection(
    collection: fastx::Collection<BoxedReader>,
    opath: Option<&str>,
    mode: BinseqMode,
    config: Config,
) -> Result<(usize, usize)> {
    let mut writer = match_output(opath)?;
    match mode {
        BinseqMode::Binseq => encode_collection_bq(collection, &mut writer, config),
        BinseqMode::VBinseq => encode_collection_vbq(collection, &mut writer, config),
    }
}

fn encode_collection_bq(
    mut collection: fastx::Collection<BoxedReader>,
    output: &mut BoxedWriter,
    config: Config,
) -> Result<(usize, usize)> {
    // Get the sequence lengths
    let (slen, xlen) = match collection.collection_type() {
        fastx::CollectionType::Single => {
            let inner = collection.inner_mut();
            let slen = get_sequence_len(&mut inner[0])?;
            (slen, 0)
        }
        fastx::CollectionType::Paired => {
            let inner = collection.inner_mut();
            let slen = get_sequence_len(&mut inner[0])?;
            let xlen = get_sequence_len(&mut inner[1])?;
            (slen, xlen)
        }
        fastx::CollectionType::Interleaved => {
            let inner = collection.inner_mut();
            let (slen, xlen) = get_interleaved_sequence_len(&mut inner[0])?;
            (slen, xlen)
        }
        _ => {
            bail!("Unsupported collection type found in `encode_collection_bq`");
        }
    };

    let header = bq::BinseqHeaderBuilder::new()
        .slen(slen)
        .xlen(xlen)
        .bitsize(config.bitsize)
        .flags(false)
        .build()?;
    let mut processor = BinseqProcessor::new(header, config.policy, output)?;
    process_collection(collection, &mut processor, config.threads)?;

    let num_records = processor.get_global_record_count();
    let num_skipped = processor.get_global_skipped_count();

    Ok((num_records, num_skipped))
}

fn encode_collection_vbq(
    collection: fastx::Collection<BoxedReader>,
    output: &mut BoxedWriter,
    config: Config,
) -> Result<(usize, usize)> {
    let quality = match collection.unique_format() {
        Some(Format::Fastq) | None => config.quality,
        Some(Format::Fasta) => false, // never record quality for fasta
    };
    let paired = match collection.collection_type() {
        fastx::CollectionType::Single => false,
        fastx::CollectionType::Paired | fastx::CollectionType::Interleaved => true,
        _ => {
            bail!("Unsupported collection type passed to `encode_collection_vbq`")
        }
    };
    let header = vbq::VBinseqHeaderBuilder::new()
        .block(config.block_size as u64)
        .qual(quality)
        .compressed(config.compress)
        .paired(paired)
        .bitsize(config.bitsize)
        .headers(config.headers)
        .flags(false)
        .build();
    let mut processor = VBinseqProcessor::new(header, config.policy, output)?;
    process_collection(collection, &mut processor, config.threads)?;
    processor.finish()?;

    let num_records = processor.get_global_record_count();
    let num_skipped = processor.get_global_skipped_count();

    Ok((num_records, num_skipped))
}

fn process_collection<P>(
    collection: fastx::Collection<BoxedReader>,
    processor: &mut P,
    threads: usize,
) -> Result<()>
where
    P: for<'a> ParallelProcessor<fastx::RefRecord<'a>>
        + for<'a> PairedParallelProcessor<fastx::RefRecord<'a>>,
{
    match collection.collection_type() {
        fastx::CollectionType::Single => {
            collection.process_parallel(processor, threads, None)?;
        }
        fastx::CollectionType::Paired => {
            collection.process_parallel_paired(processor, threads, None)?;
        }
        fastx::CollectionType::Interleaved => {
            collection.process_parallel_interleaved(processor, threads, None)?;
        }
        _ => bail!("Unsupported collection type"),
    }
    Ok(())
}

#[cfg(feature = "htslib")]
pub fn encode_htslib(
    inpath: &str,
    opath: Option<&str>,
    mode: BinseqMode,
    config: Config,
    paired: bool,
) -> Result<(usize, usize)> {
    let mut writer = match_output(opath)?;
    match mode {
        BinseqMode::Binseq => encode_htslib_bq(inpath, &mut writer, config, paired),
        BinseqMode::VBinseq => encode_htslib_vbq(inpath, &mut writer, config, paired),
    }
}

#[cfg(feature = "htslib")]
fn encode_htslib_bq(
    inpath: &str,
    output: &mut BoxedWriter,
    config: Config,
    paired: bool,
) -> Result<(usize, usize)> {
    use super::utils::get_sequence_len_htslib;
    use paraseq::{htslib, prelude::*};

    let (slen, xlen) = get_sequence_len_htslib(inpath, paired)?;
    let header = bq::BinseqHeaderBuilder::new()
        .slen(slen)
        .xlen(xlen)
        .bitsize(config.bitsize)
        .flags(false)
        .build()?;
    let reader = htslib::Reader::from_path(inpath)?;
    let mut processor = BinseqProcessor::new(header, config.policy, output)?;
    if paired {
        reader.process_parallel_interleaved(&mut processor, config.threads)
    } else {
        reader.process_parallel(&mut processor, config.threads)
    }?;

    let num_records = processor.get_global_record_count();
    let num_skipped = processor.get_global_skipped_count();

    Ok((num_records, num_skipped))
}

#[cfg(feature = "htslib")]
fn encode_htslib_vbq(
    inpath: &str,
    output: &mut BoxedWriter,
    config: Config,
    paired: bool,
) -> Result<(usize, usize)> {
    use paraseq::{htslib, prelude::*};

    let header = vbq::VBinseqHeaderBuilder::new()
        .block(config.block_size as u64)
        .qual(config.quality)
        .compressed(config.compress)
        .paired(paired)
        .bitsize(config.bitsize)
        .headers(config.headers)
        .flags(false)
        .build();
    let reader = htslib::Reader::from_path(inpath)?;
    let mut processor = VBinseqProcessor::new(header, config.policy, output)?;
    if paired {
        reader.process_parallel_interleaved(&mut processor, config.threads)
    } else {
        reader.process_parallel(&mut processor, config.threads)
    }?;
    processor.finish()?;

    let num_records = processor.get_global_record_count();
    let num_skipped = processor.get_global_skipped_count();

    Ok((num_records, num_skipped))
}
