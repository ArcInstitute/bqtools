use anyhow::Result;
use binseq::{BinseqReader, ParallelReader};
use log::trace;

use crate::cli::QcCommand;

mod base_content;
mod base_quality;
mod config;
mod dup_levels;
mod gc_content;
mod modules;
mod proc;
mod report;
mod seq_length;
mod seq_quality;

use config::QcConfig;
use modules::QcModule;

pub const PHRED_OFFSET: u8 = 33;
pub type QualAbundance = [usize; 94];
pub const DEFAULT_QUAL_ABUNDANCE: QualAbundance = [0; 94];

pub fn run(args: &QcCommand) -> Result<()> {
    let reader = BinseqReader::new(args.input.path())?;
    let paired = reader.is_paired();
    let total_records = reader.num_records()?;
    let range = args
        .input
        .span
        .map(|mut span| span.get_range(total_records))
        .transpose()?;
    let processed_records = range.as_ref().map_or(total_records, |r| r.end - r.start);

    let mut proc = proc::QcProcessor::new(
        &args.qc.outdir,
        QcConfig::from_opts(&args.qc),
        args.input.path().to_string(),
        processed_records,
        paired,
    )?;

    if let Some(range) = range {
        trace!("Processing span: {}..{}", range.start, range.end);
        reader.process_parallel_range(proc.clone(), args.qc.threads, range)?;
    } else {
        trace!("Processing all records: n={total_records}");
        reader.process_parallel(proc.clone(), args.qc.threads)?;
    }
    proc.finish()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use anyhow::Result;
    use clap::Parser;
    use tempfile::{tempdir, NamedTempFile};

    use crate::cli::BinseqMode;
    use crate::testutils::write_fastx;

    fn encode(paths: &[&Path], out_path: &Path) -> Result<()> {
        let mut args: Vec<String> = vec!["encode".into()];
        args.extend(paths.iter().map(|p| p.to_str().unwrap().to_string()));
        args.push("-o".into());
        args.push(out_path.to_str().unwrap().to_string());
        let cmd = crate::cli::EncodeCommand::try_parse_from(args)?;
        crate::commands::encode::run(&cmd)
    }

    fn run_qc(bq_path: &Path, outdir: &Path, extra_args: &[&str]) -> Result<()> {
        let mut args: Vec<String> = vec!["qc".into(), bq_path.to_str().unwrap().to_string()];
        args.extend(extra_args.iter().map(ToString::to_string));
        args.push("-o".into());
        args.push(outdir.to_str().unwrap().to_string());
        let cmd = crate::cli::QcCommand::try_parse_from(args)?;
        super::run(&cmd)
    }

    #[test]
    fn test_qc_single_end_summary_report() -> Result<()> {
        for mode in BinseqMode::enum_iter() {
            let fq = write_fastx().nrec(200).call()?;
            let bq = NamedTempFile::with_suffix(mode.extension())?;
            encode(&[fq.path()], bq.path())?;

            let outdir = tempdir()?;
            run_qc(bq.path(), outdir.path(), &[])?;

            let summary = std::fs::read_to_string(outdir.path().join("summary.md"))?;
            assert!(summary.contains("# BQtools QC Report"), "mode={mode:?}");
            assert!(summary.contains("| Reads | 200 |"), "mode={mode:?}");
            assert!(summary.contains("| Paired | false |"), "mode={mode:?}");
            assert!(
                summary.contains("## Per-Base Sequence Quality"),
                "mode={mode:?}"
            );
            assert!(summary.contains("## Per-Sequence Quality"), "mode={mode:?}");
            assert!(
                summary.contains("## Per-Base Sequence Content"),
                "mode={mode:?}"
            );
            assert!(
                summary.contains("## Per-Sequence GC Content"),
                "mode={mode:?}"
            );
            assert!(
                summary.contains("## Sequence Length Distribution"),
                "mode={mode:?}"
            );
            assert!(
                summary.contains("## Sequence Duplication Levels"),
                "mode={mode:?}"
            );

            // single-end input has no extended (R2) side, so no split headings
            assert!(!summary.contains("### R1"), "mode={mode:?}");
            assert!(!summary.contains("### R2"), "mode={mode:?}");
        }

        Ok(())
    }

    #[test]
    fn test_qc_paired_end_summary_report_splits_r1_r2() -> Result<()> {
        for mode in BinseqMode::enum_iter() {
            let r1 = write_fastx().nrec(150).call()?;
            let r2 = write_fastx().nrec(150).call()?;
            let bq = NamedTempFile::with_suffix(mode.extension())?;
            encode(&[r1.path(), r2.path()], bq.path())?;

            let outdir = tempdir()?;
            run_qc(bq.path(), outdir.path(), &[])?;

            let summary = std::fs::read_to_string(outdir.path().join("summary.md"))?;
            assert!(summary.contains("| Reads | 150 |"), "mode={mode:?}");
            assert!(summary.contains("| Paired | true |"), "mode={mode:?}");
            assert!(summary.contains("### R1"), "mode={mode:?}");
            assert!(summary.contains("### R2"), "mode={mode:?}");
        }

        Ok(())
    }

    #[test]
    fn test_qc_summary_omits_disabled_modules() -> Result<()> {
        for mode in BinseqMode::enum_iter() {
            let fq = write_fastx().nrec(100).call()?;
            let bq = NamedTempFile::with_suffix(mode.extension())?;
            encode(&[fq.path()], bq.path())?;

            let outdir = tempdir()?;
            run_qc(
                bq.path(),
                outdir.path(),
                &["--skip-dup-levels", "--skip-overrepresented"],
            )?;

            let summary = std::fs::read_to_string(outdir.path().join("summary.md"))?;
            assert!(
                !summary.contains("## Sequence Duplication Levels"),
                "mode={mode:?}"
            );
            assert!(
                !summary.contains("## Overrepresented Sequences"),
                "mode={mode:?}"
            );
            assert!(
                !outdir.path().join("duplication_levels_R1.tsv").exists(),
                "mode={mode:?}"
            );
        }

        Ok(())
    }
}
