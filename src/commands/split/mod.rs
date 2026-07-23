mod splitter;

use anyhow::Result;
use binseq::{bq, cbq, vbq, BinseqReader, BinseqWriterBuilder, ParallelReader};

#[cfg(feature = "fuzzy")]
use splitter::FuzzySplitter;
use splitter::{AhoCorasickSplitter, RegexSplitter, SplitProcessor, Splitter};

use crate::{
    cli::{BinseqMode, SplitCommand},
    commands::{
        grep::{all_patterns_fixed, PatternCollection},
        utils::make_directory,
    },
};

/// The three pattern sets a split operates over: primary-only, secondary-only,
/// and either-sequence patterns.
struct AllPatterns {
    pat1: PatternCollection,
    pat2: PatternCollection,
    pat: PatternCollection,
}
impl AllPatterns {
    pub fn are_fixed(&self) -> bool {
        all_patterns_fixed(&[&self.pat1, &self.pat2, &self.pat])
    }
}

fn load_patterns(args: &SplitCommand) -> Result<AllPatterns> {
    let (mut pat1, mut pat2, mut pat) = args.patterns.load_all_patterns()?;
    if args.split.rc {
        pat1.reverse_complement()?;
        pat2.reverse_complement()?;
        pat.reverse_complement()?;
    }
    Ok(AllPatterns { pat1, pat2, pat })
}

/// Selects and builds the splitter backend.
///
/// Fuzzy matching (`-z/--fuzzy`) takes priority when enabled. Otherwise,
/// fixed-string pattern sets use the Aho-Corasick backend (auto-detected, or
/// forced with `-x/--fixed`); anything else falls back to the regex backend.
fn build_splitter(args: &SplitCommand) -> Result<Splitter> {
    let patterns = load_patterns(args)?;

    #[cfg(feature = "fuzzy")]
    if args.fuzzy_args.fuzzy {
        log::trace!(
            "Using fuzzy splitter backend (k={}, inexact={}, backend=sassy)",
            args.fuzzy_args.distance,
            args.fuzzy_args.inexact,
        );
        let splitter = FuzzySplitter::new(
            &patterns.pat1,
            &patterns.pat2,
            &patterns.pat,
            args.fuzzy_args.distance,
            args.fuzzy_args.inexact,
            args.fuzzy_args.max_n_frac,
        )?;
        return Ok(Splitter::Fuzzy(Box::new(splitter)));
    }

    let use_fixed = args.split.fixed || patterns.are_fixed();
    if !args.split.fixed && use_fixed {
        log::debug!("All patterns are fixed strings — auto-selecting Aho-Corasick");
    }

    if use_fixed {
        log::trace!(
            "Using Aho-Corasick splitter backend (dfa={})",
            !args.split.no_dfa,
        );
        let splitter = AhoCorasickSplitter::new(
            &patterns.pat1,
            &patterns.pat2,
            &patterns.pat,
            args.split.no_dfa,
        )?;
        Ok(Splitter::AhoCorasick(splitter))
    } else {
        log::trace!("Using regex splitter backend");
        let splitter = RegexSplitter::new(&patterns.pat1, &patterns.pat2, &patterns.pat)?;
        Ok(Splitter::Regex(splitter))
    }
}

fn get_builder(args: &SplitCommand) -> Result<BinseqWriterBuilder> {
    let builder = match args.input.mode()? {
        BinseqMode::Bq => {
            let reader = bq::MmapReader::new(args.input.path())?;
            let header = reader.header();
            BinseqWriterBuilder::from_bq_header(header)
        }
        BinseqMode::Vbq => {
            let reader = vbq::MmapReader::new(args.input.path())?;
            let header = reader.header();
            BinseqWriterBuilder::from_vbq_header(header)
        }
        BinseqMode::Cbq => {
            let reader = cbq::MmapReader::new(args.input.path())?;
            let header = reader.header();
            BinseqWriterBuilder::from_cbq_header(header)
        }
    };
    Ok(builder)
}

pub fn run(args: &SplitCommand) -> Result<()> {
    args.validate()?;
    let splitter = build_splitter(args)?;
    let builder = get_builder(args)?;
    make_directory(&args.split.basepath)?;
    let mut proc = SplitProcessor::new(
        splitter,
        &builder,
        &args.split.basepath,
        args.input.mode()?,
        !args.split.skip_unmatched,
        &args.split.unmatched_basename,
    )?;
    let reader = BinseqReader::new(args.input.path())?;
    reader.process_parallel(proc.clone(), args.split.threads)?;
    proc.finish()?;
    if !args.split.quiet {
        proc.pprint_counts()?;
    }
    if args.split.min_records > 0 {
        let removed = proc.prune_below(args.split.min_records)?;
        if removed > 0 {
            log::debug!("Removed {removed} output file(s) below the record threshold");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;

    use anyhow::Result;
    use clap::Parser;
    use tempfile::NamedTempFile;

    use crate::cli::BinseqMode;
    use crate::testutils::{count_binseq, write_fastx, DEFAULT_NUM_RECORDS};

    fn encode(in_path: &std::path::Path, out_path: &std::path::Path) -> Result<()> {
        let cmd = crate::cli::EncodeCommand::try_parse_from([
            "encode",
            in_path.to_str().unwrap(),
            "-o",
            out_path.to_str().unwrap(),
        ])?;
        crate::commands::encode::run(&cmd)
    }

    /// Write a plain-text pattern file (one pattern per line).
    fn write_patterns(patterns: &[&str]) -> Result<NamedTempFile> {
        let tmp = NamedTempFile::with_suffix(".txt")?;
        let mut f = std::fs::File::create(tmp.path())?;
        for p in patterns {
            writeln!(f, "{p}")?;
        }
        Ok(tmp)
    }

    /// Sum binseq record counts across every file in `dir` with `extension`.
    fn count_all_in_dir(dir: &std::path::Path, extension: &str) -> Result<usize> {
        let ext = extension.trim_start_matches('.');
        std::fs::read_dir(dir)?
            .filter_map(std::result::Result::ok)
            .filter(|e| {
                e.path()
                    .extension()
                    .and_then(|x| x.to_str())
                    .is_some_and(|x| x == ext)
            })
            .map(|e| count_binseq(&e.path()))
            .try_fold(0usize, |acc, r| r.map(|n| acc + n))
    }

    /// The total records across all split output files must equal the input count,
    /// regardless of how records distribute across patterns.
    #[test]
    fn test_split_record_conservation() -> Result<()> {
        for mode in BinseqMode::enum_iter() {
            let in_tmp = write_fastx().call()?;
            let bq_tmp = NamedTempFile::with_suffix(mode.extension())?;
            encode(in_tmp.path(), bq_tmp.path())?;

            // Two patterns: no matter how records split, the total must equal input.
            let pat_file = write_patterns(&["AAAA", "CCCC"])?;
            let out_dir = tempfile::tempdir()?;

            let cmd = crate::cli::SplitCommand::try_parse_from([
                "split",
                bq_tmp.path().to_str().unwrap(),
                "--file",
                pat_file.path().to_str().unwrap(),
                "--basepath",
                out_dir.path().to_str().unwrap(),
                "--min-records",
                "0", // keep empty output files so we capture everything
                "--quiet",
            ])?;
            super::run(&cmd)?;

            let total = count_all_in_dir(out_dir.path(), mode.extension())?;
            assert_eq!(
                total, DEFAULT_NUM_RECORDS,
                "split total count wrong for {mode:?}"
            );
        }
        Ok(())
    }

    /// Using --skip-unmatched: no unmatched file should be created.
    /// With a universal pattern ("A"), every record should match, so the
    /// matched file contains all records.
    #[test]
    fn test_split_skip_unmatched() -> Result<()> {
        let in_tmp = write_fastx().call()?;
        let bq_tmp = NamedTempFile::with_suffix(".cbq")?;
        encode(in_tmp.path(), bq_tmp.path())?;

        // "A" is a single-character pattern that matches any sequence containing
        // an 'A'. After BQ encoding (Ns replaced), all 100-base sequences will
        // contain at least one A with overwhelming probability.
        let pat_file = write_patterns(&["A"])?;
        let out_dir = tempfile::tempdir()?;

        let cmd = crate::cli::SplitCommand::try_parse_from([
            "split",
            bq_tmp.path().to_str().unwrap(),
            "--file",
            pat_file.path().to_str().unwrap(),
            "--basepath",
            out_dir.path().to_str().unwrap(),
            "--skip-unmatched",
            "--quiet",
        ])?;
        super::run(&cmd)?;

        // Only the "A.cbq" file should exist; verify its count.
        let matched_path = out_dir.path().join("A.cbq");
        assert!(matched_path.exists(), "expected A.cbq in output dir");
        assert_eq!(count_binseq(&matched_path)?, DEFAULT_NUM_RECORDS);

        // With --skip-unmatched and a universal pattern, nothing else should be there.
        let file_count = std::fs::read_dir(out_dir.path())?.count();
        assert_eq!(file_count, 1, "expected exactly one output file");

        Ok(())
    }

    /// Named patterns (FASTA headers) become output file aliases.
    #[test]
    fn test_split_named_patterns() -> Result<()> {
        let in_tmp = write_fastx().call()?;
        let bq_tmp = NamedTempFile::with_suffix(".cbq")?;
        encode(in_tmp.path(), bq_tmp.path())?;

        // FASTA pattern: header becomes the alias → output file name
        let pat_file = {
            let tmp = NamedTempFile::with_suffix(".fasta")?;
            let mut f = std::fs::File::create(tmp.path())?;
            writeln!(f, ">universal_pattern")?;
            writeln!(f, "A")?;
            tmp
        };
        let out_dir = tempfile::tempdir()?;

        let cmd = crate::cli::SplitCommand::try_parse_from([
            "split",
            bq_tmp.path().to_str().unwrap(),
            "--file",
            pat_file.path().to_str().unwrap(),
            "--basepath",
            out_dir.path().to_str().unwrap(),
            "--skip-unmatched",
            "--quiet",
        ])?;
        super::run(&cmd)?;

        // The alias "universal_pattern" → "universal_pattern.cbq"
        let matched_path = out_dir.path().join("universal_pattern.cbq");
        assert!(matched_path.exists(), "expected universal_pattern.cbq");
        assert_eq!(count_binseq(&matched_path)?, DEFAULT_NUM_RECORDS);

        Ok(())
    }

    /// `--rc` should reverse complement file patterns before matching: a
    /// pattern file containing the reverse complement of a known substring
    /// should match reads containing that substring directly.
    #[test]
    fn test_split_rc_matches_reverse_complement() -> Result<()> {
        // "GATTACA" is not a palindrome; its reverse complement is "TGTAATC".
        let seq = "ACGTACGTGATTACAACGTACGT";
        let in_tmp = NamedTempFile::with_suffix(".fastq")?;
        {
            let mut f = std::fs::File::create(in_tmp.path())?;
            writeln!(f, "@read1")?;
            writeln!(f, "{seq}")?;
            writeln!(f, "+")?;
            writeln!(f, "{}", "I".repeat(seq.len()))?;
        }
        let bq_tmp = NamedTempFile::with_suffix(".cbq")?;
        encode(in_tmp.path(), bq_tmp.path())?;

        let pat_file = write_patterns(&["TGTAATC"])?;
        let out_dir = tempfile::tempdir()?;

        let cmd = crate::cli::SplitCommand::try_parse_from([
            "split",
            bq_tmp.path().to_str().unwrap(),
            "--file",
            pat_file.path().to_str().unwrap(),
            "--basepath",
            out_dir.path().to_str().unwrap(),
            "--skip-unmatched",
            "--quiet",
            "--rc",
        ])?;
        super::run(&cmd)?;

        // The pattern is reverse complemented to "GATTACA" before matching,
        // and the output alias reflects the RC'd sequence.
        let matched_path = out_dir.path().join("GATTACA.cbq");
        assert!(matched_path.exists(), "expected GATTACA.cbq after --rc");
        assert_eq!(count_binseq(&matched_path)?, 1);

        Ok(())
    }

    /// `--rc` must reject regex patterns, since reverse complementing a
    /// regex is undefined.
    #[test]
    fn test_split_rc_rejects_regex_pattern() -> Result<()> {
        let in_tmp = write_fastx().call()?;
        let bq_tmp = NamedTempFile::with_suffix(".cbq")?;
        encode(in_tmp.path(), bq_tmp.path())?;

        let pat_file = write_patterns(&["AC.GT"])?;
        let out_dir = tempfile::tempdir()?;

        let cmd = crate::cli::SplitCommand::try_parse_from([
            "split",
            bq_tmp.path().to_str().unwrap(),
            "--file",
            pat_file.path().to_str().unwrap(),
            "--basepath",
            out_dir.path().to_str().unwrap(),
            "--quiet",
            "--rc",
        ])?;
        assert!(
            super::run(&cmd).is_err(),
            "--rc should reject regex patterns"
        );

        Ok(())
    }
}
