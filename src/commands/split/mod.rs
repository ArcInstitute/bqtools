mod splitter;

use anyhow::Result;
use binseq::{
    bq,
    cbq::{self, MmapReader},
    vbq, BinseqWriterBuilder, ParallelReader,
};

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
    let (pat1, pat2, pat) = args.patterns.load_all_patterns()?;
    Ok(AllPatterns { pat1, pat2, pat })
}

/// Selects and builds the splitter backend.
///
/// Fixed-string pattern sets use the Aho-Corasick backend (auto-detected, or
/// forced with `-x/--fixed`); anything else falls back to the regex backend.
fn build_splitter(args: &SplitCommand) -> Result<Splitter> {
    let patterns = load_patterns(args)?;
    let use_fixed = args.split.fixed || patterns.are_fixed();
    if !args.split.fixed && use_fixed {
        log::debug!("All patterns are fixed strings — auto-selecting Aho-Corasick");
    }

    if use_fixed {
        log::trace!(
            "Using Aho-Corasick splitter backend (no_dfa={})",
            args.split.no_dfa
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
    let reader = MmapReader::new(args.input.path())?;
    reader.process_parallel(proc.clone(), args.split.threads)?;
    proc.finish()?;
    proc.pprint_counts()?;
    Ok(())
}
