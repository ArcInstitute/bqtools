mod color;
mod filter;
mod pattern_count;
mod range;

#[cfg(feature = "fuzzy")]
use filter::FuzzyMatcher;
use log::warn;
#[cfg(feature = "fuzzy")]
use pattern_count::FuzzyPatternCounter;

use filter::{FilterProcessor, PatternMatcher, RegexMatcher};
use pattern_count::{
    AhoCorasickPatternCounter, PatternCountProcessor, PatternCounter, RegexPatternCounter,
};
pub use range::SimpleRange;

use super::decode::build_writer;
use crate::{
    cli::{FileFormat, GrepCommand, Mate},
    commands::{decode::SplitWriter, grep::filter::AhoCorasickMatcher},
};

use anyhow::Result;
use binseq::prelude::*;

/// Returns true if the pattern is a fixed DNA string (only ACGT).
fn is_fixed(pattern: &[u8]) -> bool {
    !pattern.is_empty()
        && pattern
            .iter()
            .all(|b| matches!(b, b'A' | b'C' | b'G' | b'T'))
}

/// Returns true if all patterns across multiple sets are fixed DNA strings.
fn all_patterns_fixed(pattern_sets: &[&[Vec<u8>]]) -> bool {
    pattern_sets
        .iter()
        .flat_map(|s| s.iter())
        .all(|p| is_fixed(p))
}

fn build_counter(args: &GrepCommand) -> Result<PatternCounter> {
    #[cfg(feature = "fuzzy")]
    if args.grep.fuzzy_args.fuzzy {
        let counter = FuzzyPatternCounter::new(
            args.grep.bytes_pat1()?,
            args.grep.bytes_pat2()?,
            args.grep.bytes_pat()?,
            args.grep.fuzzy_args.distance,
            args.grep.fuzzy_args.inexact,
            args.grep.invert,
        );
        return Ok(PatternCounter::Fuzzy(counter));
    }

    let pat1 = args.grep.bytes_pat1()?;
    let pat2 = args.grep.bytes_pat2()?;
    let pat = args.grep.bytes_pat()?;
    let use_fixed = args.grep.fixed || all_patterns_fixed(&[&pat1, &pat2, &pat]);
    if !args.grep.fixed && use_fixed {
        log::debug!("All patterns are fixed strings — auto-selecting Aho-Corasick");
    }

    if use_fixed {
        let counter =
            AhoCorasickPatternCounter::new(pat1, pat2, pat, args.grep.no_dfa, args.grep.invert)?;
        Ok(PatternCounter::AhoCorasick(counter))
    } else {
        let counter = RegexPatternCounter::new(
            args.grep.bytes_reg1()?,
            args.grep.bytes_reg2()?,
            args.grep.bytes_reg()?,
            args.grep.invert,
        );
        Ok(PatternCounter::Regex(counter))
    }
}

fn run_pattern_count(args: &GrepCommand, reader: BinseqReader) -> Result<()> {
    let counter = build_counter(args)?;
    let proc = PatternCountProcessor::new(counter, args.grep.range);
    if let Some(mut span) = args.input.span {
        let num_records = reader.num_records()?;
        reader.process_parallel_range(
            proc.clone(),
            args.output.threads(),
            span.get_range(num_records)?,
        )?
    } else {
        reader.process_parallel(proc.clone(), args.output.threads())?;
    }
    proc.pprint_pattern_counts()?;
    Ok(())
}

fn build_matcher(args: &GrepCommand) -> Result<PatternMatcher> {
    #[cfg(feature = "fuzzy")]
    if args.grep.fuzzy_args.fuzzy {
        let matcher = FuzzyMatcher::new(
            args.grep.bytes_pat1()?,
            args.grep.bytes_pat2()?,
            args.grep.bytes_pat()?,
            args.grep.fuzzy_args.distance,
            args.grep.fuzzy_args.inexact,
            args.grep.range.map_or(0, |r| r.offset()),
        );
        return Ok(PatternMatcher::Fuzzy(matcher));
    }

    let pat1 = args.grep.bytes_pat1()?;
    let pat2 = args.grep.bytes_pat2()?;
    let pat = args.grep.bytes_pat()?;
    let use_fixed = args.grep.fixed || all_patterns_fixed(&[&pat1, &pat2, &pat]);
    if !args.grep.fixed && use_fixed {
        log::debug!("All patterns are fixed strings — auto-selecting Aho-Corasick");
    }

    if use_fixed && !args.grep.and_logic() {
        let matcher = AhoCorasickMatcher::new(
            pat1,
            pat2,
            pat,
            args.grep.no_dfa,
            args.grep.range.map_or(0, |r| r.offset()),
        )?;
        Ok(PatternMatcher::AhoCorasick(matcher))
    } else {
        if use_fixed {
            warn!("`-x/--fixed provided but ignored when using AND logic");
        }
        let matcher = RegexMatcher::new(
            args.grep.bytes_reg1()?,
            args.grep.bytes_reg2()?,
            args.grep.bytes_reg()?,
            args.grep.range.map_or(0, |r| r.offset()),
        );
        Ok(PatternMatcher::Regex(matcher))
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
    let matcher = build_matcher(args)?;
    let proc = FilterProcessor::new(
        matcher,
        args.grep.and_logic(),
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
mod fixed_detection_tests {
    use super::{all_patterns_fixed, is_fixed};

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
        let p1 = vec![b"ACGT".to_vec(), b"TTTT".to_vec()];
        let p2 = vec![b"GGGG".to_vec()];
        assert!(all_patterns_fixed(&[&p1, &p2]));
    }

    #[test]
    fn test_all_patterns_fixed_with_regex() {
        let p1 = vec![b"ACGT".to_vec(), b"AC.GT".to_vec()];
        let p2 = vec![b"GGGG".to_vec()];
        assert!(!all_patterns_fixed(&[&p1, &p2]));
    }

    #[test]
    fn test_all_patterns_fixed_empty_sets() {
        let p1: Vec<Vec<u8>> = vec![];
        let p2: Vec<Vec<u8>> = vec![];
        assert!(all_patterns_fixed(&[&p1, &p2]));
    }
}
