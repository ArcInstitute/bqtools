mod color;
mod filter;
mod pattern_count;
mod patterns;
mod range;

#[cfg(feature = "fuzzy")]
use filter::FuzzyMatcher;
use log::{error, warn};
#[cfg(feature = "fuzzy")]
use pattern_count::FuzzyPatternCounter;

use filter::{FilterProcessor, PatternMatcher, RegexMatcher};
use pattern_count::{
    AhoCorasickPatternCounter, PatternCount, PatternCountProcessor, PatternCounter,
    RegexPatternCounter,
};
pub use patterns::{Pattern, PatternCollection};
pub use range::SimpleRange;

use super::decode::build_writer;
use crate::{
    cli::{FileFormat, GrepCommand, Mate},
    commands::{decode::SplitWriter, grep::filter::AhoCorasickMatcher},
};

use anyhow::{bail, Result};
use binseq::prelude::*;

/// Returns true if the pattern is a fixed DNA string (only ACGT).
fn is_fixed(pattern: &[u8]) -> bool {
    !pattern.is_empty()
        && pattern
            .iter()
            .all(|b| matches!(b, b'A' | b'C' | b'G' | b'T'))
}

/// Returns true if all patterns across multiple sets are fixed DNA strings.
pub fn all_patterns_fixed(pattern_sets: &[&PatternCollection]) -> bool {
    pattern_sets
        .iter()
        .flat_map(|s| s.iter())
        .all(|p| is_fixed(&p.sequence))
}

/// Handles pattern mates by clearing and ingesting patterns into the appropriate collections.
///
/// This is relevant when using the `--mate` option, and enforces that no matches can occur on an ignored mate.
fn redistribute_patterns(
    pat1: &mut PatternCollection,
    pat2: &mut PatternCollection,
    pat: &mut PatternCollection,
    mate: Mate,
) -> Result<()> {
    match mate {
        Mate::Both => {
            // Do nothing - both mates are used
        }
        Mate::One => {
            pat2.clear(); // remove existing patterns from mate 2
            pat1.ingest(pat); // take patterns from both and apply only to mate 1
            if pat1.is_empty() {
                error!("No patterns provided for mate 1");
                bail!("No patterns provided for mate 1");
            }
        }
        Mate::Two => {
            pat1.clear(); // remove existing patterns from mate 1
            pat2.ingest(pat); // take patterns from both and apply only to mate 2
            if pat2.is_empty() {
                error!("No patterns provided for mate 2");
                bail!("No patterns provided for mate 2");
            }
        }
    }
    Ok(())
}

fn load_patterns(args: &GrepCommand) -> Result<AllPatterns> {
    let mut pat1 = args.grep.patterns_m1()?;
    let mut pat2 = args.grep.patterns_m2()?;
    let mut pat = args.grep.patterns()?;
    redistribute_patterns(&mut pat1, &mut pat2, &mut pat, args.output.mate)?;
    Ok(AllPatterns { pat1, pat2, pat })
}

struct AllPatterns {
    pat1: PatternCollection,
    pat2: PatternCollection,
    pat: PatternCollection,
}
impl AllPatterns {
    pub fn are_fixed(&self) -> bool {
        all_patterns_fixed(&[&self.pat1, &self.pat2, &self.pat])
    }

    /// Total number of patterns across all three collections. AND vs OR
    /// logic only changes behavior when combining 2+ patterns, so callers
    /// use this to avoid treating AND logic as active for a single pattern.
    pub fn total_len(&self) -> usize {
        self.pat1.len() + self.pat2.len() + self.pat.len()
    }
}

fn build_counter(args: &GrepCommand) -> Result<PatternCounter> {
    #[cfg(feature = "fuzzy")]
    if args.grep.fuzzy_args.fuzzy {
        let patterns = load_patterns(args)?;
        let counter = FuzzyPatternCounter::new(
            patterns.pat1,
            patterns.pat2,
            patterns.pat,
            args.grep.fuzzy_args.distance,
            args.grep.fuzzy_args.inexact,
            args.grep.invert,
            args.grep.fuzzy_args.max_n_frac,
        )?;
        return Ok(PatternCounter::Fuzzy(Box::new(counter)));
    }

    let patterns = load_patterns(args)?;
    let use_fixed = args.grep.fixed || patterns.are_fixed();
    if !args.grep.fixed && use_fixed {
        log::debug!("All patterns are fixed strings — auto-selecting Aho-Corasick");
    }

    if use_fixed {
        let counter = AhoCorasickPatternCounter::new(
            patterns.pat1,
            patterns.pat2,
            patterns.pat,
            args.grep.no_dfa,
            args.grep.invert,
        )?;
        Ok(PatternCounter::AhoCorasick(counter))
    } else {
        let counter =
            RegexPatternCounter::new(patterns.pat1, patterns.pat2, patterns.pat, args.grep.invert)?;
        Ok(PatternCounter::Regex(counter))
    }
}

fn run_pattern_count(args: &GrepCommand, reader: BinseqReader) -> Result<()> {
    let counter = build_counter(args)?;
    let pattern_names = counter.pattern_names();
    let proc = PatternCountProcessor::new(counter, args.grep.range, pattern_names);
    if let Some(mut span) = args.input.span {
        let num_records = reader.num_records()?;
        reader.process_parallel_range(
            proc.clone(),
            args.output.threads(),
            span.get_range(num_records)?,
        )?;
    } else {
        reader.process_parallel(proc.clone(), args.output.threads())?;
    }
    proc.pprint_pattern_counts()?;
    Ok(())
}

/// Builds the pattern matcher, plus the effective AND-logic flag to use with
/// it. AND vs OR only changes behavior when combining 2+ patterns, so with
/// a single pattern AND logic is downgraded to OR — this keeps Aho-Corasick
/// eligible (it doesn't implement AND) and matches the returned matcher to
/// the logic value callers must pass alongside it.
fn build_matcher(args: &GrepCommand) -> Result<(PatternMatcher, bool)> {
    #[cfg(feature = "fuzzy")]
    if args.grep.fuzzy_args.fuzzy {
        let patterns = load_patterns(args)?;
        let and_logic = args.grep.and_logic() && patterns.total_len() > 1;
        let matcher = FuzzyMatcher::new(
            &patterns.pat1.bytes(),
            &patterns.pat2.bytes(),
            &patterns.pat.bytes(),
            args.grep.fuzzy_args.distance,
            args.grep.fuzzy_args.inexact,
            args.grep.range.map_or(0, |r| r.offset()),
            args.grep.fuzzy_args.max_n_frac,
        )?;
        return Ok((PatternMatcher::Fuzzy(Box::new(matcher)), and_logic));
    }

    let patterns = load_patterns(args)?;
    let use_fixed = args.grep.fixed || patterns.are_fixed();
    if !args.grep.fixed && use_fixed {
        log::debug!("All patterns are fixed strings — auto-selecting Aho-Corasick");
    }

    // AND vs OR logic is only meaningful when combining 2+ patterns.
    let and_logic = args.grep.and_logic() && patterns.total_len() > 1;

    if use_fixed && !and_logic {
        let matcher = AhoCorasickMatcher::new(
            &patterns.pat1.bytes(),
            &patterns.pat2.bytes(),
            &patterns.pat.bytes(),
            args.grep.no_dfa,
            args.grep.range.map_or(0, |r| r.offset()),
        )?;
        Ok((PatternMatcher::AhoCorasick(matcher), and_logic))
    } else {
        if use_fixed {
            warn!("`-x/--fixed provided but ignored when using AND logic");
        }
        let matcher = RegexMatcher::new(
            patterns.pat1.regexes()?,
            patterns.pat2.regexes()?,
            patterns.pat.regexes()?,
            args.grep.range.map_or(0, |r| r.offset()),
        );
        Ok((PatternMatcher::Regex(matcher), and_logic))
    }
}

fn run_grep(
    args: &GrepCommand,
    reader: BinseqReader,
    writer: SplitWriter,
    format: FileFormat,
    mate: Option<Mate>,
) -> Result<()> {
    let count = args.grep.count || args.grep.frac;
    let (matcher, and_logic) = build_matcher(args)?;
    let proc = FilterProcessor::new(
        matcher,
        and_logic,
        args.grep.invert,
        count,
        args.grep.frac,
        args.grep.range,
        writer,
        format,
        mate,
        args.should_color(),
    );

    if let Some(mut span) = args.input.span {
        let num_records = reader.num_records()?;
        reader.process_parallel_range(
            proc.clone(),
            args.output.threads(),
            span.get_range(num_records)?,
        )?;
    } else {
        reader.process_parallel(proc.clone(), args.output.threads())?;
    }
    if count {
        proc.pprint_counts();
    }

    Ok(())
}

pub fn run(args: &GrepCommand) -> Result<()> {
    args.grep.validate()?;
    let reader = BinseqReader::new(args.input.path())?;
    let writer = build_writer(&args.output, reader.is_paired())?;
    let format = args.output.format()?;
    let mate = if reader.is_paired() {
        Some(args.output.mate())
    } else {
        None
    };

    if args.grep.pattern_count {
        run_pattern_count(args, reader)
    } else {
        run_grep(args, reader, writer, format, mate)
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use clap::Parser;
    use itertools::iproduct;
    use tempfile::NamedTempFile;

    use crate::cli::{BinseqMode, FileFormat};
    use crate::testutils::{count_fastx_records, write_fastx, DEFAULT_NUM_RECORDS};

    fn encode(in_path: &std::path::Path, out_path: &std::path::Path) -> Result<()> {
        let cmd = crate::cli::EncodeCommand::try_parse_from([
            "encode",
            in_path.to_str().unwrap(),
            "-o",
            out_path.to_str().unwrap(),
        ])?;
        crate::commands::encode::run(&cmd)
    }

    fn grep_count(bq_path: &std::path::Path, pattern: &str, invert: bool) -> Result<usize> {
        // Build a grep command that writes to a temp file and count the result.
        let out_tmp = NamedTempFile::with_suffix(".fastq")?;
        let mut args = vec![
            "grep",
            bq_path.to_str().unwrap(),
            pattern,
            "-o",
            out_tmp.path().to_str().unwrap(),
        ];
        if invert {
            args.push("-v");
        }
        let cmd = crate::cli::GrepCommand::try_parse_from(args)?;
        super::run(&cmd)?;
        count_fastx_records(out_tmp.path())
    }

    /// grep returns a count ≤ total records and > 0 for a short common pattern.
    #[test]
    fn test_grep_basic_count() -> Result<()> {
        for mode in BinseqMode::enum_iter() {
            let in_tmp = write_fastx().call()?;
            let bq_tmp = NamedTempFile::with_suffix(mode.extension())?;
            encode(in_tmp.path(), bq_tmp.path())?;

            let count = grep_count(bq_tmp.path(), "A", false)?;
            assert!(count > 0, "grep count should be > 0 for {mode:?}");
            assert!(
                count <= DEFAULT_NUM_RECORDS,
                "grep count {count} exceeds total for {mode:?}"
            );
        }
        Ok(())
    }

    /// A single fixed-string pattern under the default AND logic must not
    /// panic (Aho-Corasick doesn't support AND) and must match the count
    /// produced with explicit OR logic, since AND/OR are equivalent with
    /// only one pattern.
    #[test]
    fn test_grep_single_fixed_pattern_with_default_and_logic() -> Result<()> {
        for mode in BinseqMode::enum_iter() {
            let in_tmp = write_fastx().call()?;
            let bq_tmp = NamedTempFile::with_suffix(mode.extension())?;
            encode(in_tmp.path(), bq_tmp.path())?;

            let and_count = grep_count(bq_tmp.path(), "AAAA", false)?;

            let out_tmp = NamedTempFile::with_suffix(".fastq")?;
            let cmd = crate::cli::GrepCommand::try_parse_from([
                "grep",
                bq_tmp.path().to_str().unwrap(),
                "AAAA",
                "-o",
                out_tmp.path().to_str().unwrap(),
                "--or-logic",
            ])?;
            super::run(&cmd)?;
            let or_count = count_fastx_records(out_tmp.path())?;

            assert_eq!(
                and_count, or_count,
                "AND and OR logic should match for a single pattern, mode={mode:?}"
            );
        }
        Ok(())
    }

    /// forward matches + inverted matches must equal the total record count exactly.
    #[test]
    fn test_grep_invert_complementary() -> Result<()> {
        for (mode, fmt) in iproduct!(BinseqMode::enum_iter(), FileFormat::fastx_iter()) {
            let in_tmp = write_fastx().format(fmt).call()?;
            let bq_tmp = NamedTempFile::with_suffix(mode.extension())?;
            encode(in_tmp.path(), bq_tmp.path())?;

            let fwd = grep_count(bq_tmp.path(), "AAAA", false)?;
            let inv = grep_count(bq_tmp.path(), "AAAA", true)?;
            assert_eq!(
                fwd + inv,
                DEFAULT_NUM_RECORDS,
                "fwd({fwd}) + inv({inv}) != {DEFAULT_NUM_RECORDS} for {mode:?} {fmt:?}"
            );
        }
        Ok(())
    }

    /// grep writes matching records to a file across all (mode, format) combinations.
    #[test]
    fn test_grep_all_modes_and_formats() -> Result<()> {
        for (mode, fmt) in iproduct!(BinseqMode::enum_iter(), FileFormat::fastx_iter()) {
            let in_tmp = write_fastx().format(fmt).call()?;
            let bq_tmp = NamedTempFile::with_suffix(mode.extension())?;
            encode(in_tmp.path(), bq_tmp.path())?;

            let out_tmp = NamedTempFile::with_suffix(fmt.fastx_suffix())?;
            let cmd = crate::cli::GrepCommand::try_parse_from([
                "grep",
                bq_tmp.path().to_str().unwrap(),
                "A",
                "-o",
                out_tmp.path().to_str().unwrap(),
            ])?;
            super::run(&cmd)
                .map_err(|e| anyhow::anyhow!("grep failed for {mode:?} {fmt:?}: {e}"))?;

            let count = count_fastx_records(out_tmp.path())?;
            assert!(
                count <= DEFAULT_NUM_RECORDS,
                "grep output count {count} > total for {mode:?} {fmt:?}"
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod fixed_detection_tests {
    use crate::commands::grep::redistribute_patterns;

    use super::{all_patterns_fixed, is_fixed, Mate, Pattern, PatternCollection};

    fn pc(patterns: &[&[u8]]) -> PatternCollection {
        PatternCollection(
            patterns
                .iter()
                .map(|p| Pattern {
                    name: None,
                    sequence: p.to_vec(),
                })
                .collect(),
        )
    }

    #[test]
    fn test_fixed_dna_strings() {
        assert!(is_fixed(b"ACGTACGT"));
        assert!(is_fixed(b"AAAAAAAAAA"));
        assert!(is_fixed(b"ACGT"));
    }

    #[test]
    fn test_empty_string() {
        assert!(!is_fixed(b""));
    }

    #[test]
    fn test_iupac_ambiguity_codes() {
        assert!(!is_fixed(b"ACGTNRYW"));
        assert!(!is_fixed(b"ACGN"));
    }

    #[test]
    fn test_lowercase_not_fixed() {
        assert!(!is_fixed(b"acgt"));
    }

    #[test]
    fn test_regex_patterns_not_fixed() {
        assert!(!is_fixed(b"AC.GT"));
        assert!(!is_fixed(b"AC[GT]"));
        assert!(!is_fixed(b"A{3}"));
        assert!(!is_fixed(b"^ACGT"));
        assert!(!is_fixed(b"ACG|TGA"));
        assert!(!is_fixed(b"(ACG)"));
        assert!(!is_fixed(b"AC\\dGT"));
    }

    #[test]
    fn test_all_patterns_fixed() {
        let p1 = pc(&[b"ACGT", b"TTTT"]);
        let p2 = pc(&[b"GGGG"]);
        assert!(all_patterns_fixed(&[&p1, &p2]));
    }

    #[test]
    fn test_all_patterns_fixed_with_regex() {
        let p1 = pc(&[b"ACGT", b"AC.GT"]);
        let p2 = pc(&[b"GGGG"]);
        assert!(!all_patterns_fixed(&[&p1, &p2]));
    }

    #[test]
    fn test_all_patterns_fixed_empty_sets() {
        let p1 = pc(&[]);
        let p2 = pc(&[]);
        assert!(all_patterns_fixed(&[&p1, &p2]));
    }

    #[test]
    #[allow(clippy::similar_names)]
    fn test_redistribution_noop() {
        let mut pat1 = pc(&[b"ACGT", b"TTTT"]);
        let mut pat2 = pc(&[b"GGGG"]);
        let mut pat = pc(&[b"AC.GT"]);

        let pat1_clone = pat1.clone();
        let pat2_clone = pat2.clone();
        let either_clone = pat.clone();

        redistribute_patterns(&mut pat1, &mut pat2, &mut pat, Mate::Both).unwrap();

        assert_eq!(pat1, pat1_clone);
        assert_eq!(pat2, pat2_clone);
        assert_eq!(pat, either_clone);
    }

    #[test]
    fn test_redistribution_m1() {
        let mut pat1 = pc(&[b"ACGT", b"TTTT"]);
        let mut pat2 = pc(&[b"GGGG"]);
        let mut pat = pc(&[b"AC.GT"]);

        redistribute_patterns(&mut pat1, &mut pat2, &mut pat, Mate::One).unwrap();

        assert_eq!(pat1, pc(&[b"ACGT", b"TTTT", b"AC.GT"]));
        assert!(pat2.is_empty());
        assert!(pat.is_empty());
    }

    #[test]
    fn test_redistribution_m2() {
        let mut pat1 = pc(&[b"ACGT", b"TTTT"]);
        let mut pat2 = pc(&[b"GGGG"]);
        let mut pat = pc(&[b"AC.GT"]);

        redistribute_patterns(&mut pat1, &mut pat2, &mut pat, Mate::Two).unwrap();

        assert!(pat1.is_empty());
        assert_eq!(pat2, pc(&[b"GGGG", b"AC.GT"]));
        assert!(pat.is_empty());
    }
}
