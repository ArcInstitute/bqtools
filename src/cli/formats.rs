use clap::ValueEnum;

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileFormat {
    /// FASTA file format
    #[clap(name = "a")]
    Fasta,
    /// FASTQ file format
    #[clap(name = "q")]
    Fastq,
    /// TSV file format (decode only)
    #[clap(name = "t")]
    Tsv,
}
impl FileFormat {
    pub fn from_path(path: &str) -> Option<Self> {
        let ext = match path.split('.').next_back()? {
            "gz" => path.split('.').nth_back(1)?,
            "zst" => path.split('.').nth_back(1)?,
            ext => ext,
        };
        match ext {
            "fasta" | "fa" => Some(Self::Fasta),
            "fastq" | "fq" => Some(Self::Fastq),
            _ => None,
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Fasta => "fa",
            Self::Fastq => "fq",
            Self::Tsv => "tsv",
        }
    }
}
