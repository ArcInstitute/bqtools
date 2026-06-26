use std::sync::Arc;

use crate::cli::{FileFormat, Mate, SampleCommand};
use anyhow::Result;
use binseq::prelude::*;
use parking_lot::Mutex;
use rand::{RngExt, SeedableRng};

use super::decode::{build_writer, write_record_pair, SplitWriter};

#[derive(Clone)]
struct SampleProcessor {
    /// Sampling Options
    fraction: f64,
    rng: rand::rngs::SmallRng,

    /// Local write buffers
    mixed: Vec<u8>, // General purpose, interleaved or singlets
    left: Vec<u8>, // Used when writing pairs of files (R1/R2)
    right: Vec<u8>,

    /// Quality buffers
    squal: Vec<u8>,
    xqual: Vec<u8>,

    /// Write Options
    format: FileFormat,
    mate: Option<Mate>,
    is_split: bool,

    /// Global values
    global_writer: Arc<Mutex<SplitWriter>>,
}
impl SampleProcessor {
    pub fn new(
        fraction: f64,
        seed: u64,
        writer: SplitWriter,
        format: FileFormat,
        mate: Option<Mate>,
    ) -> Self {
        Self {
            fraction,
            format,
            mate,
            rng: rand::rngs::SmallRng::seed_from_u64(seed),
            mixed: Vec::new(),
            left: Vec::new(),
            right: Vec::new(),
            squal: Vec::new(),
            xqual: Vec::new(),
            is_split: writer.is_split(),
            global_writer: Arc::new(Mutex::new(writer)),
        }
    }
    pub fn include_record(&mut self) -> bool {
        self.rng.random_bool(self.fraction)
    }
}
impl ParallelProcessor for SampleProcessor {
    fn process_record<B: BinseqRecord>(&mut self, record: B) -> binseq::Result<()> {
        let sbuf = record.sseq();
        let xbuf = record.xseq();

        if self.include_record() {
            let squal = if record.has_quality() {
                record.squal()
            } else {
                if self.squal.len() < sbuf.len() {
                    self.squal.resize(sbuf.len(), b'?');
                }
                &self.squal
            };

            let xqual = if record.is_paired() {
                if record.has_quality() {
                    record.xqual()
                } else {
                    if self.xqual.len() < xbuf.len() {
                        self.xqual.resize(xbuf.len(), b'?');
                    }
                    &self.xqual
                }
            } else {
                if self.xqual.len() < xbuf.len() {
                    self.xqual.resize(xbuf.len(), b'?');
                }
                &self.xqual
            };

            write_record_pair(
                &mut self.left,
                &mut self.right,
                &mut self.mixed,
                self.mate,
                self.is_split,
                sbuf,
                squal,
                record.sheader(),
                xbuf,
                xqual,
                record.xheader(),
                self.format,
            )?;
        }

        Ok(())
    }

    fn on_batch_complete(&mut self) -> binseq::Result<()> {
        // Lock the mutex to write to the global buffer
        {
            let mut writer = self.global_writer.lock();
            if writer.is_split() {
                writer.write_split(&self.left, true)?;
                writer.write_split(&self.right, false)?;
            } else {
                writer.write_interleaved(&self.mixed)?;
            }
            writer.flush()?;
        }

        // Clear the local buffer and reset the local record count
        self.mixed.clear();
        self.left.clear();
        self.right.clear();
        Ok(())
    }
}

pub fn run(args: &SampleCommand) -> Result<()> {
    args.sample.validate()?;
    let reader = BinseqReader::new(args.input.path())?;
    let writer = build_writer(&args.output, reader.is_paired())?;
    let format = args.output.format()?;
    let mate = if reader.is_paired() {
        Some(args.output.mate())
    } else {
        None
    };
    let proc = SampleProcessor::new(args.sample.fraction, args.sample.seed, writer, format, mate);
    if let Some(mut span) = args.input.span {
        let num_records = reader.num_records()?;
        reader.process_parallel_range(
            proc.clone(),
            args.output.threads(),
            span.get_range(num_records)?,
        )?;
    } else {
        reader.process_parallel(proc.clone(), args.output.threads())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use clap::Parser;
    use itertools::iproduct;
    use tempfile::NamedTempFile;

    use crate::cli::{BinseqMode, FileFormat};
    use crate::testutils::{count_fastx_records, write_fastx};

    fn encode(in_path: &std::path::Path, out_path: &std::path::Path) -> Result<()> {
        let cmd = crate::cli::EncodeCommand::try_parse_from([
            "encode",
            in_path.to_str().unwrap(),
            "-o",
            out_path.to_str().unwrap(),
        ])?;
        crate::commands::encode::run(&cmd)
    }

    fn sample(
        bq_path: &std::path::Path,
        out_path: &std::path::Path,
        fraction: f64,
        seed: u64,
    ) -> Result<()> {
        let cmd = crate::cli::SampleCommand::try_parse_from([
            "sample",
            bq_path.to_str().unwrap(),
            "-F",
            &fraction.to_string(),
            "-S",
            &seed.to_string(),
            "-o",
            out_path.to_str().unwrap(),
        ])?;
        super::run(&cmd)
    }

    /// Sampling at 0.5 should produce approximately half the records (±20%).
    #[test]
    fn test_sample_half() -> Result<()> {
        let nrec = 1000;
        let fraction = 0.5_f64;
        let expected = (nrec as f64 * fraction) as usize;
        let tolerance = nrec / 5;

        for (mode, fmt) in iproduct!(BinseqMode::enum_iter(), FileFormat::fastx_iter()) {
            let in_tmp = write_fastx().format(fmt).nrec(nrec).call()?;
            let bq_tmp = NamedTempFile::with_suffix(mode.extension())?;
            encode(in_tmp.path(), bq_tmp.path())?;

            let out_tmp = NamedTempFile::with_suffix(fmt.fastx_suffix())?;
            sample(bq_tmp.path(), out_tmp.path(), fraction, 42)?;

            let count = count_fastx_records(out_tmp.path())?;
            assert!(
                count.abs_diff(expected) <= tolerance,
                "sample count {count} far from expected {expected} (±{tolerance}) for {mode:?} {fmt:?}"
            );
        }
        Ok(())
    }

    /// Sampling at fraction=1.0 must return all records exactly.
    #[test]
    fn test_sample_fraction_one() -> Result<()> {
        let nrec = 200;
        for mode in BinseqMode::enum_iter() {
            let in_tmp = write_fastx().nrec(nrec).call()?;
            let bq_tmp = NamedTempFile::with_suffix(mode.extension())?;
            encode(in_tmp.path(), bq_tmp.path())?;

            let out_tmp = NamedTempFile::with_suffix(".fastq")?;
            sample(bq_tmp.path(), out_tmp.path(), 1.0, 42)?;

            assert_eq!(
                count_fastx_records(out_tmp.path())?,
                nrec,
                "sample fraction=1.0 should return all records for {mode:?}"
            );
        }
        Ok(())
    }

    /// Different seeds should (very likely) produce different sample sizes.
    #[test]
    fn test_sample_different_seeds_vary() -> Result<()> {
        let nrec = 1000;
        let in_tmp = write_fastx().nrec(nrec).call()?;
        let bq_tmp = NamedTempFile::with_suffix(".cbq")?;
        encode(in_tmp.path(), bq_tmp.path())?;

        let counts: Vec<usize> = [42_u64, 123, 999]
            .iter()
            .map(|&seed| {
                let out_tmp = NamedTempFile::with_suffix(".fastq")?;
                sample(bq_tmp.path(), out_tmp.path(), 0.5, seed)?;
                count_fastx_records(out_tmp.path())
            })
            .collect::<Result<_>>()?;

        let unique: std::collections::HashSet<_> = counts.iter().collect();
        assert!(unique.len() > 1, "all seeds produced the same count — suspicious: {counts:?}");
        Ok(())
    }
}
