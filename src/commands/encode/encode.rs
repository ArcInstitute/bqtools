use anyhow::{bail, Result};
use binseq::{BinseqWriterBuilder, BitSize, Policy};
use log::trace;
use paraseq::{
    fastx::{self},
    prelude::{PairedParallelProcessor, ParallelProcessor},
};

use crate::{
    cli::{BinseqMode, OutputBinseq},
    commands::{
        encode::{
            processor::Encoder,
            utils::{get_interleaved_sequence_len, get_sequence_len},
        },
        match_output,
    },
    types::BoxedReader,
};

pub struct Config {
    compress: bool,
    quality: bool,
    block_size: usize,
    policy: Policy,
    bitsize: BitSize,
    headers: bool,
    threads: usize,
    compression_level: i32,
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
            compression_level: output.level,
        }
    }
}

pub fn encode_collection(
    mut collection: fastx::Collection<BoxedReader>,
    opath: Option<&str>,
    mode: BinseqMode,
    mut config: Config,
) -> Result<(usize, usize)> {
    if let Some(infmt) = collection.unique_format() {
        if infmt == fastx::Format::Fasta {
            config.quality = false;
        }
    } else {
        bail!("All input files must have the same format.");
    };
    let ohandle = match_output(opath)?;
    let mut builder = BinseqWriterBuilder::new(mode.into())
        .block_size(config.block_size)
        .compression(config.compress)
        .compression_level(config.compression_level)
        .headers(config.headers)
        .quality(config.quality)
        .policy(config.policy)
        .bitsize(config.bitsize);

    if !matches!(collection.collection_type(), fastx::CollectionType::Single) {
        builder = builder.paired(true);
    }

    // insert the slen and xlen on the builder for BQ
    if matches!(mode, BinseqMode::Bq) {
        match collection.collection_type() {
            fastx::CollectionType::Single => {
                let inner = collection.inner_mut();
                let slen = get_sequence_len(&mut inner[0])?;
                builder = builder.slen(slen as u32);
            }
            fastx::CollectionType::Paired => {
                let inner = collection.inner_mut();
                let slen = get_sequence_len(&mut inner[0])?;
                let xlen = get_sequence_len(&mut inner[1])?;
                builder = builder.slen(slen as u32).xlen(xlen as u32)
            }
            fastx::CollectionType::Interleaved => {
                let inner = collection.inner_mut();
                let (slen, xlen) = get_interleaved_sequence_len(&mut inner[0])?;
                builder = builder.slen(slen as u32).xlen(xlen as u32)
            }
            _ => {
                bail!("Unsupported collection type found in `encode_collection_bq`");
            }
        }
    }
    let writer = builder.build(ohandle)?;
    let mut processor = super::processor::Encoder::new(writer)?;
    process_collection(collection, &mut processor, config.threads)?;
    processor.finish()?;

    Ok((
        processor.get_global_record_count(),
        processor.get_global_skip_count(),
    ))
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
    use super::utils::get_sequence_len_htslib;
    use paraseq::{htslib, prelude::*};

    let ohandle = match_output(opath)?;
    let mut builder = BinseqWriterBuilder::new(mode.into())
        .block_size(config.block_size)
        .compression_level(config.compression_level)
        .headers(config.headers)
        .quality(config.quality)
        .policy(config.policy)
        .bitsize(config.bitsize)
        .paired(paired);

    if matches!(mode, BinseqMode::Bq) {
        let (slen, xlen) = get_sequence_len_htslib(inpath, paired)?;
        builder = builder.slen(slen).xlen(xlen);
    }
    let reader = htslib::Reader::from_path(inpath)?;
    let writer = builder.build(ohandle)?;
    let mut processor = Encoder::new(writer)?;
    if paired {
        reader.process_parallel_interleaved(&mut processor, config.threads)
    } else {
        reader.process_parallel(&mut processor, config.threads)
    }?;
    processor.finish()?;

    Ok((
        processor.get_global_record_count(),
        processor.get_global_skip_count(),
    ))
}

// #[cfg(feature = "htslib")]
// fn encode_htslib_bq(
//     inpath: &str,
//     output: &mut BoxedWriter,
//     config: Config,
//     paired: bool,
// ) -> Result<(usize, usize)> {
//     use super::utils::get_sequence_len_htslib;
//     use paraseq::{htslib, prelude::*};

//     trace!("Encoding htslib {inpath} into bq");

//     let (slen, xlen) = get_sequence_len_htslib(inpath, paired)?;
//     trace!("sequence length: slen={slen}, xlen={xlen}");
//     let header = bq::BinseqHeaderBuilder::new()
//         .slen(slen)
//         .xlen(xlen)
//         .bitsize(config.bitsize)
//         .flags(false)
//         .build()?;
//     let reader = htslib::Reader::from_path(inpath)?;
//     let mut processor = BqEncoder::new(header, config.policy, output)?;
//     if paired {
//         reader.process_parallel_interleaved(&mut processor, config.threads)
//     } else {
//         reader.process_parallel(&mut processor, config.threads)
//     }?;

//     let num_records = processor.get_global_record_count();
//     let num_skipped = processor.get_global_skipped_count();

//     Ok((num_records, num_skipped))
// }

// #[cfg(feature = "htslib")]
// fn encode_htslib_vbq(
//     inpath: &str,
//     output: &mut BoxedWriter,
//     config: Config,
//     paired: bool,
// ) -> Result<(usize, usize)> {
//     use paraseq::{htslib, prelude::*};
//     trace!("Encoding htslib {inpath} into vbq");

//     let header = vbq::VBinseqHeaderBuilder::new()
//         .block(config.block_size as u64)
//         .qual(config.quality)
//         .compressed(config.compress)
//         .paired(paired)
//         .bitsize(config.bitsize)
//         .headers(config.headers)
//         .flags(false)
//         .build();
//     let reader = htslib::Reader::from_path(inpath)?;
//     let mut processor = VbqEncoder::new(header, config.policy, output)?;
//     if paired {
//         reader.process_parallel_interleaved(&mut processor, config.threads)
//     } else {
//         reader.process_parallel(&mut processor, config.threads)
//     }?;
//     processor.finish()?;

//     let num_records = processor.get_global_record_count();
//     let num_skipped = processor.get_global_skipped_count();

//     Ok((num_records, num_skipped))
// }

// #[cfg(feature = "htslib")]
// fn encode_htslib_cbq(
//     inpath: &str,
//     output: &mut BoxedWriter,
//     config: Config,
//     paired: bool,
// ) -> Result<(usize, usize)> {
//     use paraseq::{htslib, prelude::*};
//     trace!("Encoding htslib {inpath} into cbq");

//     let header = cbq::FileHeaderBuilder::default()
//         .with_qualities(config.quality)
//         .is_paired(paired)
//         .with_headers(config.headers)
//         .with_block_size(config.block_size)
//         .with_flags(false)
//         .build();

//     let reader = htslib::Reader::from_path(inpath)?;
//     let mut processor = CbqEncoder::new(header, output)?;
//     if paired {
//         reader.process_parallel_interleaved(&mut processor, config.threads)
//     } else {
//         reader.process_parallel(&mut processor, config.threads)
//     }?;
//     processor.finish()?;

//     let num_records = processor.get_global_record_count();
//     let num_skipped = processor.get_global_skipped_count();

//     Ok((num_records, num_skipped))
// }
