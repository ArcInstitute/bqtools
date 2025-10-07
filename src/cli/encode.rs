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
    pub fn mode(&self) -> anyhow::Result<BinseqMode> {
        if let Some(mode) = self.output.mode {
            Ok(mode)
        } else if self.input.recursive {
            Ok(BinseqMode::VBinseq)
        } else {
            self.output.mode()
        }
    }
}
