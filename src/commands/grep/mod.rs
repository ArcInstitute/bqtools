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

    if args.grep.fixed {
        let counter = AhoCorasickPatternCounter::new(
            args.grep.bytes_pat1()?,
            args.grep.bytes_pat2()?,
            args.grep.bytes_pat()?,
            args.grep.no_dfa,
            args.grep.invert,
        )?;
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

    if args.grep.fixed && !args.grep.and_logic() {
        let matcher = AhoCorasickMatcher::new(
            args.grep.bytes_pat1()?,
            args.grep.bytes_pat2()?,
            args.grep.bytes_pat()?,
            args.grep.no_dfa,
            args.grep.range.map_or(0, |r| r.offset()),
        )?;
        Ok(PatternMatcher::AhoCorasick(matcher))
    } else {
        if args.grep.fixed {
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
