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
        if let Some(mode) = self.output.options.mode {
            Ok(mode)
        } else if self.input.recursive {
            Ok(BinseqMode::default())
        } else {
            self.output.mode()
        }
    }
    pub fn output_path(&self) -> Result<Option<String>> {
        if let Some(path) = &self.output.output {
            Ok(Some(path.clone()))
        } else if self.output.pipe {
            Ok(None)
        } else if self.input.is_stdin() {
            error!("Output path must be provided if using stdin");
            bail!("Output path must be provided if using stdin")
        } else if self.input.num_files() > if self.input.paired() { 2 } else { 1 } {
            error!("Output path must be provided if collating multiple files");
            bail!("Output path must be provided if collating multiple files")
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

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::EncodeCommand;

    fn output_path(args: &[&str]) -> anyhow::Result<Option<String>> {
        let mut argvec = vec!["encode"];
        argvec.extend_from_slice(args);
        EncodeCommand::try_parse_from(argvec)?.output_path()
    }

    /// Two single-end files collated without `-o` have no natural output name,
    /// so this must request an output path rather than falling through to `single_path()`.
    #[test]
    fn test_output_path_collate_two_files_requires_output() {
        let err = output_path(&["a.fq", "b.fq", "--collate"]).unwrap_err();
        assert!(err
            .to_string()
            .contains("Output path must be provided if collating multiple files"));
    }

    #[test]
    fn test_output_path_two_files_auto_names_pair() {
        let path = output_path(&["sample_R1.fq", "sample_R2.fq"]).unwrap();
        assert_eq!(path.as_deref(), Some("sample.cbq"));
    }

    #[test]
    fn test_output_path_collate_single_pair_auto_names() {
        let path = output_path(&["sample_R1.fq", "sample_R2.fq", "--paired", "--collate"]).unwrap();
        assert_eq!(path.as_deref(), Some("sample.cbq"));
    }

    #[test]
    fn test_output_path_collate_uses_explicit_output() {
        let path = output_path(&["a.fq", "b.fq", "--collate", "-o", "out.cbq"]).unwrap();
        assert_eq!(path.as_deref(), Some("out.cbq"));
    }
}
