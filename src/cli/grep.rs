use anyhow::{bail, Result};
use clap::Parser;

use crate::{cli::FileFormat, commands::grep::SimpleRange};

use super::{InputBinseq, OutputFile};

/// Grep a BINSEQ file and output to FASTQ or FASTA.
#[derive(Parser, Debug)]
pub struct GrepCommand {
    #[clap(flatten)]
    pub input: InputBinseq,

    #[clap(flatten)]
    pub output: OutputFile,

    #[clap(flatten)]
    pub grep: GrepArgs,
}
impl GrepCommand {
    pub fn should_color(&self) -> bool {
        match self.output.format() {
            Ok(FileFormat::Bam) => false,
            _ => {
                self.output.output.is_none()
                    && self.output.prefix.is_none()
                    && self.grep.color.should_color()
            }
        }
    }
}

#[derive(Parser, Debug)]
#[clap(next_help_heading = "SEARCH OPTIONS")]
pub struct GrepArgs {
    /// Regex expression to search for in primary sequence
    #[clap(short = 'r', long)]
    pub reg1: Vec<String>,

    /// Regex expression to search for in extended sequence
    #[clap(short = 'R', long)]
    pub reg2: Vec<String>,

    /// Regex expression to search for in either sequence
    pub reg: Vec<String>,

    /// Invert pattern criteria (like grep -v)
    #[clap(short = 'v', long)]
    pub invert: bool,

    /// Only count matches
    #[clap(short = 'C', long, conflicts_with = "pattern_count")]
    pub count: bool,

    /// Only match patterns that are within this range.
    ///
    /// Will not match if the pattern is outside the range or if
    /// the sequence cannot be sliced within the range (i.e. out of bounds).
    ///
    /// Examples: --range=0..100, --range=..30, --range=60..
    #[clap(long)]
    pub range: Option<SimpleRange>,

    /// Count number of matches per pattern
    ///
    /// This will output a TSV with the number of matches per pattern.
    /// Note that a sequence may contribute to multiple patterns counts.
    /// A pattern will also only be counted once per sequence.
    #[clap(short = 'P', long, conflicts_with = "count")]
    pub pattern_count: bool,

    /// use OR logic for multiple patterns (default=AND)
    #[clap(long, conflicts_with = "pattern_count")]
    or_logic: bool,

    /// Colorize output (auto, always, never)
    #[clap(
        long,
        value_name = "WHEN",
        default_value = "auto",
        conflicts_with = "format"
    )]
    color: ColorWhen,

    #[cfg(feature = "fuzzy")]
    #[clap(flatten)]
    pub fuzzy_args: FuzzyArgs,

    #[clap(flatten)]
    pub file_args: PatternFileArgs,
}

impl GrepArgs {
    pub fn validate(&self) -> Result<()> {
        if self.reg1.is_empty()
            && self.reg2.is_empty()
            && self.reg.is_empty()
            && self.file_args.empty()
        {
            anyhow::bail!("At least one pattern must be specified");
        }
        Ok(())
    }
    fn chain_regex(
        &self,
        cli_patterns: &[String],
        filetype: PatternFileType,
    ) -> Result<Vec<regex::bytes::Regex>> {
        let mut all_patterns = cli_patterns
            .iter()
            .map(|s| s.to_owned())
            .collect::<Vec<String>>();
        if !self.file_args.empty_file(filetype) {
            all_patterns.extend(self.file_args.read_file_patterns(filetype)?);
        }

        // for AND logic all patterns are kept as is
        if self.and_logic() {
            Ok(all_patterns
                .iter()
                .map(|s| {
                    regex::bytes::Regex::new(s).expect("Could not build regex from pattern: {s}")
                })
                .collect())

        // for OR logic they can be compiled into a single regex for performance
        } else {
            let global_pattern = all_patterns.join("|");
            if global_pattern.is_empty() {
                Ok(vec![])
            } else {
                Ok(vec![regex::bytes::Regex::new(&global_pattern).expect(
                    "Could not build regex from pattern: {global_pattern}",
                )])
            }
        }
    }
    pub fn bytes_reg1(&self) -> Result<Vec<regex::bytes::Regex>> {
        self.chain_regex(&self.reg1, PatternFileType::SFile)
    }
    pub fn bytes_reg2(&self) -> Result<Vec<regex::bytes::Regex>> {
        self.chain_regex(&self.reg2, PatternFileType::XFile)
    }
    pub fn bytes_reg(&self) -> Result<Vec<regex::bytes::Regex>> {
        self.chain_regex(&self.reg, PatternFileType::File)
    }
    pub fn and_logic(&self) -> bool {
        if self.file_args.empty() {
            !self.or_logic
        } else {
            // using any FILE args forces OR logic
            false
        }
    }
}

#[cfg(feature = "fuzzy")]
impl GrepArgs {
    fn chain_bytes(
        &self,
        cli_patterns: &[String],
        filetype: PatternFileType,
    ) -> Result<Vec<Vec<u8>>> {
        let bytes_iter = cli_patterns.iter().map(|s| s.as_bytes().to_vec());
        if self.file_args.empty_file(filetype) {
            Ok(bytes_iter.collect())
        } else {
            let patterns = self.file_args.patterns(filetype)?;
            Ok(bytes_iter.chain(patterns.into_iter()).collect())
        }
    }
    pub fn bytes_pat1(&self) -> Result<Vec<Vec<u8>>> {
        self.chain_bytes(&self.reg1, PatternFileType::SFile)
    }
    pub fn bytes_pat2(&self) -> Result<Vec<Vec<u8>>> {
        self.chain_bytes(&self.reg2, PatternFileType::XFile)
    }
    pub fn bytes_pat(&self) -> Result<Vec<Vec<u8>>> {
        self.chain_bytes(&self.reg, PatternFileType::File)
    }
}

#[cfg(feature = "fuzzy")]
#[derive(Parser, Debug)]
#[clap(next_help_heading = "FUZZY MATCHING OPTIONS")]
pub struct FuzzyArgs {
    /// Fuzzy finding using `sassy`
    ///
    /// Note that regex expressions are not supported with this flag.
    #[clap(short = 'z', long)]
    pub fuzzy: bool,

    /// Maximum edit distance to allow when fuzzy matching
    ///
    /// Only used with fuzzy matching
    #[clap(short = 'k', long, default_value = "1")]
    pub distance: usize,

    /// Only return inexact matches on fuzzy matching
    ///
    /// This will capture matches that are not exact, but are within the specified edit distance.
    #[clap(short = 'i', long)]
    pub inexact: bool,
}

#[derive(Parser, Debug)]
#[clap(next_help_heading = "PATTERN FILE OPTIONS")]
pub struct PatternFileArgs {
    /// File of patterns to search for
    ///
    /// This assumes one pattern per line.
    /// Patterns may be regex or literal (fuzzy doesn't support regex).
    /// These will match against either primary or extended sequence.
    #[clap(long)]
    pub file: Option<String>,

    /// File of patterns to search for in primary sequence
    ///
    /// This assumes one pattern per line.
    /// Patterns may be regex or literal (fuzzy doesn't support regex).
    #[clap(long)]
    pub sfile: Option<String>,

    /// File of patterns to search for in extended sequence
    ///
    /// This assumes one pattern per line.
    /// Patterns may be regex or literal (fuzzy doesn't support regex).
    #[clap(long)]
    pub xfile: Option<String>,
}
impl PatternFileArgs {
    fn empty(&self) -> bool {
        self.file.is_none() && self.sfile.is_none() && self.xfile.is_none()
    }

    fn empty_file(&self, filetype: PatternFileType) -> bool {
        match filetype {
            PatternFileType::File => self.file.is_none(),
            PatternFileType::SFile => self.sfile.is_none(),
            PatternFileType::XFile => self.xfile.is_none(),
        }
    }

    fn read_file(&self, filetype: PatternFileType) -> Result<String> {
        let file = match filetype {
            PatternFileType::File => &self.file,
            PatternFileType::SFile => &self.sfile,
            PatternFileType::XFile => &self.xfile,
        };
        if let Some(file) = file {
            Ok(std::fs::read_to_string(file)?)
        } else {
            bail!("Specified file type {:?} not provided at CLI", filetype)
        }
    }

    fn read_file_patterns(&self, filetype: PatternFileType) -> Result<Vec<String>> {
        let contents = self.read_file(filetype)?;
        Ok(contents.lines().map(|line| line.to_string()).collect())
    }

    fn patterns(&self, filetype: PatternFileType) -> Result<Vec<Vec<u8>>> {
        let contents = self.read_file(filetype)?;
        let mut patterns = Vec::new();
        for line in contents.lines() {
            patterns.push(line.as_bytes().to_vec());
        }
        Ok(patterns)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum PatternFileType {
    /// patterns for either primary or extended sequence
    File,
    /// primary sequence patterns
    SFile,
    /// extended sequence patterns
    XFile,
}

#[derive(Clone, Debug, clap::ValueEnum)]
pub enum ColorWhen {
    Auto,
    Always,
    Never,
}

impl ColorWhen {
    pub fn should_color(&self) -> bool {
        match self {
            ColorWhen::Always => true,
            ColorWhen::Never => false,
            ColorWhen::Auto => {
                use is_terminal::IsTerminal;
                std::io::stdout().is_terminal()
            }
        }
    }
}
