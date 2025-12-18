use anyhow::{bail, Result};
use binseq::{bq, cbq, vbq, BitSize, Policy};
use log::trace;
use paraseq::{
    fastx::{self, Format},
    prelude::{PairedParallelProcessor, ParallelProcessor},
};

use crate::{
    cli::{BinseqMode, OutputBinseq},
    commands::{
        encode::{
            processor::{BqEncoder, CbqEncoder, VbqEncoder},
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
            headers: output.headers(),
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
        BinseqMode::Bq => encode_collection_bq(collection, &mut writer, config),
        BinseqMode::Vbq => encode_collection_vbq(collection, &mut writer, config),
        BinseqMode::Cbq => encode_collection_cbq(collection, &mut writer, config),
    }
}

fn encode_collection_bq(
    mut collection: fastx::Collection<BoxedReader>,
    output: &mut BoxedWriter,
    config: Config,
) -> Result<(usize, usize)> {
    trace!("Encoding collection into bq");
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
    trace!("sequence length: slen={slen}, xlen={xlen}");

    let header = bq::BinseqHeaderBuilder::new()
        .slen(slen)
        .xlen(xlen)
        .bitsize(config.bitsize)
        .flags(false)
        .build()?;
    let mut processor = BqEncoder::new(header, config.policy, output)?;
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
    let mut processor = VbqEncoder::new(header, config.policy, output)?;
    process_collection(collection, &mut processor, config.threads)?;
    processor.finish()?;

    let num_records = processor.get_global_record_count();
    let num_skipped = processor.get_global_skipped_count();

    Ok((num_records, num_skipped))
}

fn encode_collection_cbq(
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
            bail!("Unsupported collection type passed to `encode_collection_cbq`")
        }
    };
    let header = cbq::FileHeaderBuilder::default()
        .with_qualities(quality)
        .is_paired(paired)
        .with_headers(config.headers)
        .with_flags(false)
        .build();
    let mut processor = CbqEncoder::new(header, output)?;
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
            trace!(
                "Processing single collection of size {}",
                collection.inner().len()
            );
            collection.process_parallel(processor, threads, None)?;
        }
        fastx::CollectionType::Paired => {
            trace!(
                "Processing paired collection of size {}",
                collection.inner().len()
            );
            collection.process_parallel_paired(processor, threads, None)?;
        }
        fastx::CollectionType::Interleaved => {
            trace!(
                "Processing interleaved collection of size {}",
                collection.inner().len()
            );
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
        BinseqMode::Bq => encode_htslib_bq(inpath, &mut writer, config, paired),
        BinseqMode::Vbq => encode_htslib_vbq(inpath, &mut writer, config, paired),
        BinseqMode::Cbq => encode_htslib_cbq(inpath, &mut writer, config, paired),
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

    trace!("Encoding htslib {inpath} into bq");

    let (slen, xlen) = get_sequence_len_htslib(inpath, paired)?;
    trace!("sequence length: slen={slen}, xlen={xlen}");
    let header = bq::BinseqHeaderBuilder::new()
        .slen(slen)
        .xlen(xlen)
        .bitsize(config.bitsize)
        .flags(false)
        .build()?;
    let reader = htslib::Reader::from_path(inpath)?;
    let mut processor = BqEncoder::new(header, config.policy, output)?;
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
    trace!("Encoding htslib {inpath} into vbq");

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
    let mut processor = VbqEncoder::new(header, config.policy, output)?;
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

#[cfg(feature = "htslib")]
fn encode_htslib_cbq(
    inpath: &str,
    output: &mut BoxedWriter,
    config: Config,
    paired: bool,
) -> Result<(usize, usize)> {
    use paraseq::{htslib, prelude::*};
    trace!("Encoding htslib {inpath} into cbq");

    let header = cbq::FileHeaderBuilder::default()
        .with_qualities(config.quality)
        .is_paired(paired)
        .with_headers(config.headers)
        .with_flags(false)
        .build();

    let reader = htslib::Reader::from_path(inpath)?;
    let mut processor = CbqEncoder::new(header, output)?;
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
