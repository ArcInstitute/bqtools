use anyhow::{bail, Result};
use log::{error, trace};

use crate::commands::encode::utils::generate_output_name;

use super::{BinseqMode, InputFile, OutputBinseq};

#[derive(clap::Parser, Debug, Clone)]
/// Encode FASTQ or FASTA files to BINSEQ.
pub struct EncodeCommand {
    #[clap(flatten)]
    pub input: InputFile,

    #[clap(flatten)]
    pub output: OutputBinseq,
}
impl EncodeCommand {
    pub fn mode(&self) -> Result<BinseqMode> {
        if let Some(mode) = self.output.mode {
            Ok(mode)
        } else if self.input.recursive {
            Ok(BinseqMode::VBinseq)
        } else {
            self.output.mode()
        }
    }
    pub fn output_path(&self) -> Result<Option<String>> {
        if let Some(path) = &self.output.output {
            Ok(Some(path.to_string()))
        } else if self.output.pipe {
            Ok(None)
        } else if self.input.is_stdin() {
            error!("Output path must be provided if using stdin");
            bail!("Output path must be provided if using stdin")
        } else {
            let outpath = if self.input.paired() {
                let (r1, r2) = self.input.paired_paths()?;
                generate_output_name(&[r1.into(), r2.into()], self.mode()?.extension())?
            } else {
                let path = self.input.single_path()?.unwrap();
                generate_output_name(&[path.into()], self.mode()?.extension())?
            };
            trace!("Auto-determined outpath path: {outpath}");
            Ok(Some(outpath))
        }
    }
}
