use std::path::Path;
use std::process::Command;

use anyhow::Result;
use bon::builder;
use itertools::iproduct;

mod common;
use common::{
    output_tempfile, write_fastx, BinseqMode, CompressionStatus, FastxFormat, COMMAND_NAME,
};

#[builder]
fn run_encode_then_decode(
    in_path: &Path,
    out_path: &Path,
    decode_out_path: &Path,
    threads: Option<usize>,
    #[builder(default)] vbq_index: bool,
    #[builder(default)] vbq_vcomp: bool,
    #[builder(default)] vbq_skip_qual: bool,
    decode_format: Option<&str>,
    decode_mate: Option<&str>,
) -> Result<bool> {
    // First encode
    let mut encode_args: Vec<_> = vec![
        "encode",
        in_path.to_str().unwrap(),
        "-o",
        out_path.to_str().unwrap(),
    ]
    .into_iter()
    .map(|x| x.to_string())
    .collect();

    if let Some(t) = threads {
        encode_args.push("-T".to_string());
        encode_args.push(format!("{}", t));
    }
    if vbq_index {
        encode_args.push("--index".to_string());
    }
    if vbq_vcomp {
        encode_args.push("--uncompressed".to_string());
    }
    if vbq_skip_qual {
        encode_args.push("--skip-quality".to_string());
    }

    let encode_status = Command::new(COMMAND_NAME)
        .args(encode_args)
        .output()?
        .status
        .success();
    if !encode_status {
        return Ok(false);
    }

    // Then decode
    let mut decode_args: Vec<_> = vec![
        "decode",
        out_path.to_str().unwrap(),
        "-o",
        decode_out_path.to_str().unwrap(),
    ]
    .into_iter()
    .map(|x| x.to_string())
    .collect();

    if let Some(format) = decode_format {
        decode_args.push("-f".to_string());
        decode_args.push(format.to_string());
    }
    if let Some(mate) = decode_mate {
        decode_args.push("-m".to_string());
        decode_args.push(mate.to_string());
    }
    if let Some(t) = threads {
        decode_args.push("-T".to_string());
        decode_args.push(format!("{}", t));
    }
    println!("{decode_args:#?}");

    let decode_status = Command::new(COMMAND_NAME)
        .args(decode_args)
        .output()?
        .status
        .success();
    Ok(decode_status)
}

#[builder]
fn run_decode_paired_prefix(
    binseq_path: &Path,
    prefix: &str,
    decode_format: Option<&str>,
    threads: Option<usize>,
) -> Result<bool> {
    let mut decode_args: Vec<_> = vec!["decode", binseq_path.to_str().unwrap(), "--prefix", prefix]
        .into_iter()
        .map(|x| x.to_string())
        .collect();

    if let Some(format) = decode_format {
        decode_args.push("-f".to_string());
        decode_args.push(format.to_string());
    }
    if let Some(t) = threads {
        decode_args.push("-T".to_string());
        decode_args.push(format!("{}", t));
    }

    let decode_status = Command::new(COMMAND_NAME)
        .args(decode_args)
        .output()?
        .status
        .success();
    Ok(decode_status)
}

#[test]
fn test_round_trip_decode() -> Result<()> {
    for (mode, comp, format, threads) in iproduct!(
        BinseqMode::enum_iter(),
        CompressionStatus::enum_iter(),
        FastxFormat::enum_iter(),
        [None, Some(1), Some(4)],
    ) {
        let in_tmp = write_fastx().format(format).comp(comp).call()?;
        let binseq_tmp = output_tempfile(mode)?;
        let decode_tmp = tempfile::NamedTempFile::with_suffix(format.suffix())?;

        eprintln!(
            "Testing round-trip: {:?} {:?} {:?} -T {:?}",
            mode, comp, format, threads
        );

        let status = run_encode_then_decode()
            .in_path(in_tmp.path())
            .out_path(binseq_tmp.path())
            .decode_out_path(decode_tmp.path())
            .maybe_threads(threads)
            .call()?;

        assert!(
            status,
            "Round-trip failed for {:?} {:?} {:?}",
            mode, comp, format
        );
    }
    Ok(())
}

#[test]
fn test_decode_formats() -> Result<()> {
    // Test decoding to different output formats
    for (mode, output_format) in iproduct!(
        BinseqMode::enum_iter(),
        ["a", "q", "t"], // fasta, fastq, tsv
    ) {
        let in_tmp = write_fastx().format(FastxFormat::Fastq).call()?;
        let binseq_tmp = output_tempfile(mode)?;
        let decode_tmp = tempfile::NamedTempFile::new()?;

        eprintln!("Testing decode format: {:?} -> {}", mode, output_format);

        let status = run_encode_then_decode()
            .in_path(in_tmp.path())
            .out_path(binseq_tmp.path())
            .decode_out_path(decode_tmp.path())
            .decode_format(output_format)
            .call()?;

        assert!(
            status,
            "Decode format test failed for {:?} -> {}",
            mode, output_format
        );
    }
    Ok(())
}

#[test]
fn test_decode_mate_selection() -> Result<()> {
    // Create paired-end input data
    let r1_tmp = write_fastx().format(FastxFormat::Fastq).call()?;
    let r2_tmp = write_fastx().format(FastxFormat::Fastq).call()?;

    for mode in BinseqMode::enum_iter() {
        let binseq_tmp = output_tempfile(mode)?;

        // First encode paired data
        let encode_status = Command::new(COMMAND_NAME)
            .args([
                "encode",
                r1_tmp.path().to_str().unwrap(),
                r2_tmp.path().to_str().unwrap(),
                "-o",
                binseq_tmp.path().to_str().unwrap(),
            ])
            .output()?
            .status
            .success();

        assert!(encode_status, "Failed to encode paired data for {:?}", mode);

        // Test decoding specific mates
        for mate in ["1", "2", "both"] {
            let decode_tmp = tempfile::NamedTempFile::with_suffix(FastxFormat::default().suffix())?;

            let decode_status = Command::new(COMMAND_NAME)
                .args([
                    "decode",
                    binseq_tmp.path().to_str().unwrap(),
                    "-o",
                    decode_tmp.path().to_str().unwrap(),
                    "-m",
                    mate,
                ])
                .output()?
                .status
                .success();

            assert!(decode_status, "Decode mate {} failed for {:?}", mate, mode);
        }
    }
    Ok(())
}

#[test]
fn test_decode_paired_prefix() -> Result<()> {
    // Create paired-end input data
    let r1_tmp = write_fastx().format(FastxFormat::Fastq).call()?;
    let r2_tmp = write_fastx().format(FastxFormat::Fastq).call()?;

    for mode in BinseqMode::enum_iter() {
        let binseq_tmp = output_tempfile(mode)?;

        // First encode paired data
        let encode_status = Command::new(COMMAND_NAME)
            .args([
                "encode",
                r1_tmp.path().to_str().unwrap(),
                r2_tmp.path().to_str().unwrap(),
                "-o",
                binseq_tmp.path().to_str().unwrap(),
            ])
            .output()?
            .status
            .success();

        assert!(encode_status, "Failed to encode paired data for {:?}", mode);

        // Test decoding with prefix (creates separate R1/R2 files)
        let temp_dir = tempfile::tempdir()?;
        let prefix = temp_dir.path().join("test_prefix");

        let status = run_decode_paired_prefix()
            .binseq_path(binseq_tmp.path())
            .prefix(prefix.to_str().unwrap())
            .decode_format("q")
            .call()?;

        assert!(status, "Decode with prefix failed for {:?}", mode);

        // Check that both R1 and R2 files were created
        let r1_path = format!("{}_R1.fq", prefix.to_str().unwrap());
        let r2_path = format!("{}_R2.fq", prefix.to_str().unwrap());

        assert!(Path::new(&r1_path).exists(), "R1 file not created");
        assert!(Path::new(&r2_path).exists(), "R2 file not created");
    }
    Ok(())
}

#[test]
fn test_decode_threading() -> Result<()> {
    let in_tmp = write_fastx()
        .format(FastxFormat::Fastq)
        .nrec(1000) // Larger dataset for threading test
        .call()?;

    for (mode, threads) in iproduct!(BinseqMode::enum_iter(), [1, 2, 4, 8]) {
        let binseq_tmp = output_tempfile(mode)?;
        let decode_tmp = tempfile::NamedTempFile::with_suffix(FastxFormat::default().suffix())?;

        eprintln!(
            "Testing decode threading: {:?} with {} threads",
            mode, threads
        );

        let status = run_encode_then_decode()
            .in_path(in_tmp.path())
            .out_path(binseq_tmp.path())
            .decode_out_path(decode_tmp.path())
            .threads(threads)
            .call()?;

        assert!(
            status,
            "Decode threading test failed for {:?} with {} threads",
            mode, threads
        );
    }
    Ok(())
}

#[test]
fn test_decode_compressed_output() -> Result<()> {
    let in_tmp = write_fastx().format(FastxFormat::Fastq).call()?;

    for mode in BinseqMode::enum_iter() {
        let binseq_tmp = output_tempfile(mode)?;
        let decode_tmp = tempfile::NamedTempFile::with_suffix(".fastq.gz")?;

        eprintln!("Testing decode with compressed output: {:?}", mode);

        // Test with --compress flag
        let encode_status = Command::new(COMMAND_NAME)
            .args([
                "encode",
                in_tmp.path().to_str().unwrap(),
                "-o",
                binseq_tmp.path().to_str().unwrap(),
            ])
            .output()?
            .status
            .success();

        assert!(encode_status);

        let decode_status = Command::new(COMMAND_NAME)
            .args([
                "decode",
                binseq_tmp.path().to_str().unwrap(),
                "-o",
                decode_tmp.path().to_str().unwrap(),
                "--compress",
            ])
            .output()?
            .status
            .success();

        assert!(
            decode_status,
            "Decode with compression failed for {:?}",
            mode
        );
    }
    Ok(())
}
