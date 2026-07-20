use clap::ValueEnum;

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileFormat {
    /// FASTA file format
    #[clap(name = "a")]
    Fasta,
    /// FASTQ file format
    #[clap(name = "q")]
    Fastq,
    /// BAM file format
    #[clap(name = "b")]
    Bam,
    /// TSV file format (decode only)
    #[clap(name = "t")]
    Tsv,
}
impl FileFormat {
    pub fn from_path(path: &str) -> Option<Self> {
        let last = path.split('.').next_back()?.to_ascii_lowercase();
        let ext = match last.as_str() {
            "gz" | "zst" => path.split('.').nth_back(1)?.to_ascii_lowercase(),
            _ => last,
        };
        match ext.as_str() {
            "fasta" | "fa" => Some(Self::Fasta),
            "fastq" | "fq" => Some(Self::Fastq),
            "sam" | "bam" | "cram" => Some(Self::Bam),
            "tsv" | "txt" => Some(Self::Tsv),
            _ => None,
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Fasta => "fa",
            Self::Fastq => "fq",
            Self::Tsv => "tsv",
            Self::Bam => "bam",
        }
    }

    #[cfg(test)]
    pub fn fastx_iter() -> impl Iterator<Item = Self> + Clone {
        [Self::Fasta, Self::Fastq].into_iter()
    }

    #[cfg(test)]
    pub fn fastx_suffix(self) -> &'static str {
        match self {
            Self::Fasta => ".fasta",
            Self::Fastq => ".fastq",
            _ => panic!("no test suffix for non-fastx format"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::FileFormat;

    #[test]
    fn from_path_fasta() {
        assert_eq!(
            FileFormat::from_path("reads.fasta"),
            Some(FileFormat::Fasta)
        );
        assert_eq!(FileFormat::from_path("reads.fa"), Some(FileFormat::Fasta));
    }

    #[test]
    fn from_path_fastq() {
        assert_eq!(
            FileFormat::from_path("reads.fastq"),
            Some(FileFormat::Fastq)
        );
        assert_eq!(FileFormat::from_path("reads.fq"), Some(FileFormat::Fastq));
    }

    #[test]
    fn from_path_bam() {
        assert_eq!(FileFormat::from_path("reads.sam"), Some(FileFormat::Bam));
        assert_eq!(FileFormat::from_path("reads.bam"), Some(FileFormat::Bam));
        assert_eq!(FileFormat::from_path("reads.cram"), Some(FileFormat::Bam));
    }

    #[test]
    fn from_path_tsv() {
        assert_eq!(FileFormat::from_path("reads.tsv"), Some(FileFormat::Tsv));
        assert_eq!(FileFormat::from_path("reads.txt"), Some(FileFormat::Tsv));
    }

    #[test]
    fn from_path_unknown_extension() {
        assert_eq!(FileFormat::from_path("reads.bin"), None);
        assert_eq!(FileFormat::from_path("reads"), None);
    }

    #[test]
    fn from_path_strips_gzip_suffix() {
        assert_eq!(
            FileFormat::from_path("reads.fastq.gz"),
            Some(FileFormat::Fastq)
        );
        assert_eq!(
            FileFormat::from_path("reads.fa.gz"),
            Some(FileFormat::Fasta)
        );
        assert_eq!(FileFormat::from_path("reads.tsv.gz"), Some(FileFormat::Tsv));
        assert_eq!(FileFormat::from_path("reads.txt.gz"), Some(FileFormat::Tsv));
    }

    #[test]
    fn from_path_strips_zstd_suffix() {
        assert_eq!(
            FileFormat::from_path("reads.fastq.zst"),
            Some(FileFormat::Fastq)
        );
        assert_eq!(
            FileFormat::from_path("reads.fa.zst"),
            Some(FileFormat::Fasta)
        );
        assert_eq!(
            FileFormat::from_path("reads.tsv.zst"),
            Some(FileFormat::Tsv)
        );
        assert_eq!(
            FileFormat::from_path("reads.txt.zst"),
            Some(FileFormat::Tsv)
        );
    }

    #[test]
    fn from_path_handles_multi_dotted_names() {
        assert_eq!(
            FileFormat::from_path("sample.1.trimmed.fastq"),
            Some(FileFormat::Fastq)
        );
        assert_eq!(
            FileFormat::from_path("sample.1.trimmed.tsv.gz"),
            Some(FileFormat::Tsv)
        );
    }

    #[test]
    fn from_path_with_directory_components() {
        assert_eq!(
            FileFormat::from_path("/tmp/output/reads.txt"),
            Some(FileFormat::Tsv)
        );
        assert_eq!(
            FileFormat::from_path("./relative/path/reads.fq.gz"),
            Some(FileFormat::Fastq)
        );
    }

    #[test]
    fn from_path_compression_suffix_without_known_extension() {
        // Only the compression suffix present with no recognizable format extension.
        assert_eq!(FileFormat::from_path("reads.gz"), None);
        assert_eq!(FileFormat::from_path("reads.zst"), None);
    }

    #[test]
    fn from_path_is_case_insensitive() {
        assert_eq!(
            FileFormat::from_path("reads.FASTA"),
            Some(FileFormat::Fasta)
        );
        assert_eq!(FileFormat::from_path("reads.Fa"), Some(FileFormat::Fasta));
        assert_eq!(
            FileFormat::from_path("reads.FASTQ"),
            Some(FileFormat::Fastq)
        );
        assert_eq!(FileFormat::from_path("reads.Fq"), Some(FileFormat::Fastq));
        assert_eq!(FileFormat::from_path("reads.SAM"), Some(FileFormat::Bam));
        assert_eq!(FileFormat::from_path("reads.Bam"), Some(FileFormat::Bam));
        assert_eq!(FileFormat::from_path("reads.CRAM"), Some(FileFormat::Bam));
        assert_eq!(FileFormat::from_path("reads.TSV"), Some(FileFormat::Tsv));
        assert_eq!(FileFormat::from_path("reads.TXT"), Some(FileFormat::Tsv));
        assert_eq!(FileFormat::from_path("reads.Txt"), Some(FileFormat::Tsv));
    }

    #[test]
    fn from_path_case_insensitive_compression_suffix() {
        // Both the format extension and the compression suffix should match
        // regardless of case.
        assert_eq!(
            FileFormat::from_path("reads.FASTQ.GZ"),
            Some(FileFormat::Fastq)
        );
        assert_eq!(
            FileFormat::from_path("reads.TSV.Zst"),
            Some(FileFormat::Tsv)
        );
        assert_eq!(FileFormat::from_path("reads.Txt.GZ"), Some(FileFormat::Tsv));
        assert_eq!(
            FileFormat::from_path("reads.Fa.ZST"),
            Some(FileFormat::Fasta)
        );
    }
}
