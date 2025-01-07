use clap::ValueEnum;

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileFormat {
    #[clap(name = "a")]
    Fasta,
    #[clap(name = "q")]
    Fastq,
}
impl FileFormat {
    pub fn from_path(path: &str) -> Option<Self> {
        let ext = match path.split('.').last()? {
            "gz" => path.split('.').nth_back(1)?,
            ext => ext,
        };
        match ext {
            "fasta" | "fa" => Some(Self::Fasta),
            "fastq" | "fq" => Some(Self::Fastq),
            _ => None,
        }
    }

    pub fn extension(&self) -> &'static str {
        match self {
            Self::Fasta => "fa",
            Self::Fastq => "fq",
        }
    }
}
