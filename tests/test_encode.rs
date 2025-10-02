use std::path::Path;
use std::process::Command;

use anyhow::Result;
use bon::builder;
use itertools::iproduct;

mod common;
use common::{
    count_binseq, output_tempfile, write_fastx, BinseqMode, CompressionStatus, FastxFormat,
    COMMAND_NAME, DEFAULT_NUM_RECORDS,
};

#[builder]
fn run_encode(
    in_path: &Path,
    out_path: &Path,
    threads: Option<usize>,
    #[builder(default)] vbq_vcomp: bool,
    #[builder(default)] vbq_skip_qual: bool,
) -> Result<bool> {
    let mut args: Vec<_> = vec![
        "encode",
        in_path.to_str().unwrap(),
        "-o",
        out_path.to_str().unwrap(),
    ]
    .into_iter()
    .map(|x| x.to_string())
    .collect();
    if let Some(t) = threads {
        args.push("-T".to_string());
        args.push(format!("{}", t));
    }
    if vbq_vcomp {
        args.push("--uncompressed".to_string());
    }
    if vbq_skip_qual {
        args.push("--skip-quality".to_string());
    }
    eprintln!("Args: {args:#?}");
    let cmd = Command::new(COMMAND_NAME).args(args).output()?;
    Ok(cmd.status.success())
}

#[test]
fn test_encoding() -> Result<()> {
    for (mode, comp, format, threads) in iproduct!(
        BinseqMode::enum_iter(),
        CompressionStatus::enum_iter(),
        FastxFormat::enum_iter(),
        [None, Some(1), Some(0)],
    ) {
        let in_tmp = write_fastx().format(format).comp(comp).call()?;
        let out_tmp = output_tempfile(mode)?;
        eprintln!(
            "Testing: {:?} {:?} {:?} -T {:?}",
            mode, comp, format, threads
        );
        let status = run_encode()
            .in_path(in_tmp.path())
            .out_path(out_tmp.path())
            .maybe_threads(threads)
            .call()?;
        assert!(status);
        assert_eq!(count_binseq(out_tmp.path())?, DEFAULT_NUM_RECORDS);
    }
    Ok(())
}

#[test]
fn test_vbq_specialization() -> Result<()> {
    for (comp, format, vcomp, skip_qual) in iproduct!(
        CompressionStatus::enum_iter(),
        FastxFormat::enum_iter(),
        [false, true],
        [false, true],
    ) {
        let in_tmp = write_fastx().format(format).comp(comp).call()?;
        let out_tmp = output_tempfile(BinseqMode::Vbq)?;
        let status = run_encode()
            .in_path(in_tmp.path())
            .out_path(out_tmp.path())
            .vbq_vcomp(vcomp)
            .vbq_skip_qual(skip_qual)
            .call()?;
        assert!(status);
        assert_eq!(count_binseq(out_tmp.path())?, DEFAULT_NUM_RECORDS);
    }
    Ok(())
}
