mod color;
mod regex_proc;

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
    let proc = FuzzyProcessor::new(
        args.grep.bytes_pat1(),
        args.grep.bytes_pat2(),
        args.grep.bytes_pat(),
        args.grep.fuzzy_args.distance,
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
    Ok(())
}

fn run_regex(
    args: &GrepCommand,
    reader: BinseqReader,
    writer: SplitWriter,
    format: FileFormat,
    mate: Option<Mate>,
) -> Result<()> {
    let proc = RegexProcessor::new(
        args.grep.bytes_reg1(),
        args.grep.bytes_reg2(),
        args.grep.bytes_reg(),
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
    Ok(())
}

#[cfg(feature = "fuzzy")]
mod fuzzy_proc;
#[cfg(feature = "fuzzy")]
use fuzzy_proc::GrepProcessor as FuzzyProcessor;

use regex_proc::GrepProcessor as RegexProcessor;

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
    {
        if args.grep.fuzzy_args.fuzzy {
            run_fuzzy(args, reader, writer, format, mate)
        } else {
            run_regex(args, reader, writer, format, mate)
        }
    }

    #[cfg(not(feature = "fuzzy"))]
    {
        run_regex(args, reader, writer, format, mate)
    }
}
