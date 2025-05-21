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
fn run_encode(in_path: &Path, out_path: &Path, threads: Option<usize>) -> Result<bool> {
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
        eprintln!("Testing: {:?} {:?} {:?}", mode, comp, format);
        let status = run_encode()
            .in_path(in_tmp.path())
            .out_path(out_tmp.path())
            .maybe_threads(threads)
            .call()?;
        assert!(status);
    }
    assert!(false);
    Ok(())
}
