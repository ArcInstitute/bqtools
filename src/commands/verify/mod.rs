mod processor;

use anyhow::{bail, Result};
use binseq::{BinseqReader, ParallelReader};
use serde::Serialize;

use crate::cli::{Mate, VerifyCommand, VerifyOptions};
use processor::{FieldMask, VerifyProcessor};

fn field_mask(opts: &VerifyOptions) -> Result<FieldMask> {
    let fields = FieldMask {
        seq: !opts.skip_seq,
        qual: !opts.skip_qual,
        headers: !opts.skip_headers,
        flags: !opts.skip_flags,
    };
    if !(fields.seq || fields.qual || fields.headers || fields.flags) {
        bail!(
            "At least one field must be included in the checksum \
             (seq, qual, headers, flags cannot all be skipped)"
        );
    }
    Ok(fields)
}

fn mate_label(mate: Mate) -> &'static str {
    match mate {
        Mate::One => "1",
        Mate::Two => "2",
        Mate::Both => "both",
    }
}

fn field_labels(fields: FieldMask) -> Vec<&'static str> {
    let mut labels = Vec::new();
    if fields.seq {
        labels.push("seq");
    }
    if fields.qual {
        labels.push("qual");
    }
    if fields.headers {
        labels.push("headers");
    }
    if fields.flags {
        labels.push("flags");
    }
    labels
}

struct VerifyResult {
    fields: FieldMask,
    checksum: u64,
    num_records: usize,
}

/// Runs the checksum computation without printing, so it can be reused by tests.
fn compute(args: &VerifyCommand) -> Result<VerifyResult> {
    let fields = field_mask(&args.opts)?;

    let reader = BinseqReader::new(args.input.path())?;
    if args.opts.mate == Mate::Two && !reader.is_paired() {
        bail!(
            "`--mate/-M 2` was requested but `{}` is single-channel (no extended/mate-2 \
             sequence); the checksum would be computed over no fields",
            args.input.path()
        );
    }

    let processor = VerifyProcessor::new(fields, args.opts.mate);

    if let Some(mut span) = args.input.span {
        let num_records = reader.num_records()?;
        reader.process_parallel_range(
            processor.clone(),
            args.opts.threads,
            span.get_range(num_records)?,
        )?;
    } else {
        reader.process_parallel(processor.clone(), args.opts.threads)?;
    }

    Ok(VerifyResult {
        fields,
        checksum: processor.checksum(),
        num_records: processor.num_records(),
    })
}

#[derive(Serialize)]
struct VerifyReport {
    path: String,
    algorithm: &'static str,
    fields: Vec<&'static str>,
    mate: &'static str,
    num_records: usize,
    checksum: String,
}

pub fn run(args: &VerifyCommand) -> Result<()> {
    let result = compute(args)?;

    if args.opts.json {
        let report = VerifyReport {
            path: args.input.path().to_string(),
            algorithm: "xxh3-64/wrapping-sum",
            fields: field_labels(result.fields),
            mate: mate_label(args.opts.mate),
            num_records: result.num_records,
            checksum: format!("{:016x}", result.checksum),
        };
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!(
            "{:016x}\t{}\t{}",
            result.checksum,
            result.num_records,
            args.input.path()
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use clap::Parser;
    use itertools::iproduct;
    use tempfile::NamedTempFile;

    use crate::cli::BinseqMode;
    use crate::testutils::write_fastx;

    fn encode(in_path: &std::path::Path, out_path: &std::path::Path) -> Result<()> {
        let cmd = crate::cli::EncodeCommand::try_parse_from([
            "encode",
            in_path.to_str().unwrap(),
            "-o",
            out_path.to_str().unwrap(),
        ])?;
        crate::commands::encode::run(&cmd)
    }

    fn checksum(path: &std::path::Path, extra: &[&str]) -> Result<u64> {
        let mut cmd_args = vec!["verify".to_string(), path.to_str().unwrap().to_string()];
        cmd_args.extend(extra.iter().map(std::string::ToString::to_string));
        let cmd = crate::cli::VerifyCommand::try_parse_from(cmd_args)?;
        Ok(super::compute(&cmd)?.checksum)
    }

    /// Re-encoding the same input twice (independent parallel runs, so record
    /// order between the two outputs is not guaranteed) must produce the
    /// same checksum.
    #[test]
    fn test_verify_stable_across_independent_encodes() -> Result<()> {
        for mode in BinseqMode::enum_iter() {
            let in_tmp = write_fastx().call()?;

            let bq_a = NamedTempFile::with_suffix(mode.extension())?;
            encode(in_tmp.path(), bq_a.path())?;
            let bq_b = NamedTempFile::with_suffix(mode.extension())?;
            encode(in_tmp.path(), bq_b.path())?;

            let checksum_a = checksum(bq_a.path(), &[])?;
            let checksum_b = checksum(bq_b.path(), &[])?;
            assert_eq!(
                checksum_a, checksum_b,
                "checksum differed across independent encodes for {mode:?}"
            );
        }
        Ok(())
    }

    /// A corrupted byte in the encoded payload must change the checksum
    /// (when the file still parses after the corruption).
    #[test]
    fn test_verify_detects_content_change() -> Result<()> {
        let in_tmp = write_fastx().call()?;
        let bq_tmp = NamedTempFile::with_suffix(".cbq")?;
        encode(in_tmp.path(), bq_tmp.path())?;
        let original = checksum(bq_tmp.path(), &[])?;

        let mut bytes = std::fs::read(bq_tmp.path())?;
        let mid = bytes.len() / 2;
        bytes[mid] ^= 0xFF;
        std::fs::write(bq_tmp.path(), &bytes)?;

        if let Ok(changed) = checksum(bq_tmp.path(), &[]) {
            assert_ne!(original, changed, "bit flip was not detected");
        }
        Ok(())
    }

    /// `--skip-*` flags must actually change which fields feed the checksum.
    #[test]
    fn test_verify_skip_flags_change_checksum() -> Result<()> {
        let in_tmp = write_fastx().call()?;
        let bq_tmp = NamedTempFile::with_suffix(".cbq")?;
        encode(in_tmp.path(), bq_tmp.path())?;

        let full = checksum(bq_tmp.path(), &[])?;
        let skip_headers = checksum(bq_tmp.path(), &["--skip-headers"])?;
        let skip_qual = checksum(bq_tmp.path(), &["--skip-qual"])?;

        assert_ne!(full, skip_headers);
        assert_ne!(full, skip_qual);
        assert_ne!(skip_headers, skip_qual);
        Ok(())
    }

    /// Skipping every field is rejected up front.
    #[test]
    fn test_verify_rejects_empty_field_selection() {
        let cmd = crate::cli::VerifyCommand::try_parse_from([
            "verify",
            "input.cbq",
            "--skip-seq",
            "--skip-qual",
            "--skip-headers",
            "--skip-flags",
        ])
        .unwrap();
        assert!(super::field_mask(&cmd.opts).is_err());
    }

    /// Paired files restricted to a single mate must differ from the
    /// checksum over both mates.
    #[test]
    fn test_verify_mate_selection_changes_checksum() -> Result<()> {
        for mode in BinseqMode::enum_iter() {
            let r1 = write_fastx().call()?;
            let r2 = write_fastx().call()?;
            let bq_tmp = NamedTempFile::with_suffix(mode.extension())?;
            let cmd = crate::cli::EncodeCommand::try_parse_from([
                "encode",
                r1.path().to_str().unwrap(),
                r2.path().to_str().unwrap(),
                "-o",
                bq_tmp.path().to_str().unwrap(),
            ])?;
            crate::commands::encode::run(&cmd)?;

            let both = checksum(bq_tmp.path(), &[])?;
            let mate1 = checksum(bq_tmp.path(), &["-M", "1"])?;
            let mate2 = checksum(bq_tmp.path(), &["-M", "2"])?;

            assert_ne!(both, mate1, "mode={mode:?}");
            assert_ne!(both, mate2, "mode={mode:?}");
            assert_ne!(mate1, mate2, "mode={mode:?}");
        }
        Ok(())
    }

    /// Requesting mate 2 on a single-channel (unpaired) file must hard-error
    /// rather than silently produce a checksum over no fields. Mate 1 and
    /// mate "both" are unaffected, since the primary channel always exists.
    #[test]
    fn test_verify_rejects_mate_two_on_single_channel_file() -> Result<()> {
        let in_tmp = write_fastx().call()?;
        let bq_tmp = NamedTempFile::with_suffix(".cbq")?;
        encode(in_tmp.path(), bq_tmp.path())?;

        let err = checksum(bq_tmp.path(), &["-M", "2"]).unwrap_err();
        assert!(err.to_string().contains("--mate/-M 2"));

        assert!(checksum(bq_tmp.path(), &["-M", "1"]).is_ok());
        assert!(checksum(bq_tmp.path(), &["-M", "both"]).is_ok());
        Ok(())
    }

    /// `verify::run` must not error for any mode or output option.
    #[test]
    fn test_verify_run_all_modes() -> Result<()> {
        for (mode, json_flag) in iproduct!(BinseqMode::enum_iter(), [&[][..], &["--json"]]) {
            let in_tmp = write_fastx().call()?;
            let bq_tmp = NamedTempFile::with_suffix(mode.extension())?;
            encode(in_tmp.path(), bq_tmp.path())?;

            let mut args = vec![
                "verify".to_string(),
                bq_tmp.path().to_str().unwrap().to_string(),
            ];
            args.extend(json_flag.iter().map(std::string::ToString::to_string));
            let cmd = crate::cli::VerifyCommand::try_parse_from(args)?;
            super::run(&cmd)?;
        }
        Ok(())
    }
}
