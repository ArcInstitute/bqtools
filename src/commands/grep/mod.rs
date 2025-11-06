mod color;
mod filter;
mod pattern_count;

#[cfg(feature = "fuzzy")]
use filter::FuzzyMatcher;
#[cfg(feature = "fuzzy")]
use pattern_count::FuzzyPatternCounter;

use filter::{FilterProcessor, RegexMatcher};
use pattern_count::{PatternCountProcessor, RegexPatternCounter};

use super::decode::build_writer;
use crate::{
    cli::{FileFormat, GrepCommand, Mate},
    commands::decode::SplitWriter,
};

use anyhow::Result;
use binseq::prelude::*;

#[cfg(feature = "fuzzy")]
fn run_fuzzy(
    args: &GrepCommand,
    reader: BinseqReader,
    writer: SplitWriter,
    format: FileFormat,
    mate: Option<Mate>,
) -> Result<()> {
    if args.grep.pattern_count {
        let counter = FuzzyPatternCounter::new(
            args.grep.bytes_pat1()?,
            args.grep.bytes_pat2()?,
            args.grep.bytes_pat()?,
            args.grep.fuzzy_args.distance,
            args.grep.fuzzy_args.inexact,
            args.grep.invert,
        );
        let proc = PatternCountProcessor::new(counter);
        reader.process_parallel(proc.clone(), args.output.threads())?;
        proc.pprint_pattern_counts()?;
    } else {
        let matcher = FuzzyMatcher::new(
            args.grep.bytes_pat1()?,
            args.grep.bytes_pat2()?,
            args.grep.bytes_pat()?,
            args.grep.fuzzy_args.distance,
            args.grep.fuzzy_args.inexact,
        );
        let proc = FilterProcessor::new(
            matcher,
            args.grep.and_logic(),
            args.grep.invert,
            args.grep.count,
            writer,
            format,
            mate,
            args.should_color(),
        );
        reader.process_parallel(proc.clone(), args.output.threads())?;
        if args.grep.count {
            proc.pprint_counts();
        }
    }

    Ok(())
}

fn run_regex(
    args: &GrepCommand,
    reader: BinseqReader,
    writer: SplitWriter,
    format: FileFormat,
    mate: Option<Mate>,
) -> Result<()> {
    if args.grep.pattern_count {
        let counter = RegexPatternCounter::new(
            args.grep.bytes_reg1()?,
            args.grep.bytes_reg2()?,
            args.grep.bytes_reg()?,
            args.grep.invert,
        );
        let proc = PatternCountProcessor::new(counter);
        reader.process_parallel(proc.clone(), args.output.threads())?;
        proc.pprint_pattern_counts()?;
    } else {
        let matcher = RegexMatcher::new(
            args.grep.bytes_reg1()?,
            args.grep.bytes_reg2()?,
            args.grep.bytes_reg()?,
        );
        let proc = FilterProcessor::new(
            matcher,
            args.grep.and_logic(),
            args.grep.invert,
            args.grep.count,
            writer,
            format,
            mate,
            args.should_color(),
        );
        reader.process_parallel(proc.clone(), args.output.threads())?;
        if args.grep.count {
            proc.pprint_counts();
        }
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

    #[cfg(feature = "fuzzy")]
    if args.grep.fuzzy_args.fuzzy {
        return run_fuzzy(args, reader, writer, format, mate);
    }
    run_regex(args, reader, writer, format, mate)
}
