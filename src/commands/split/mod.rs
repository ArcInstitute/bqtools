use std::{
    collections::HashMap,
    io::{stderr, Write},
    path::Path,
    sync::Arc,
};

use aho_corasick::{AhoCorasick, AhoCorasickBuilder, AhoCorasickKind};
use anyhow::Result;
use binseq::{
    bq,
    cbq::{self, MmapReader},
    vbq, BinseqWriter, BinseqWriterBuilder, ParallelProcessor, ParallelReader,
    SequencingRecordBuilder,
};
use fixedbitset::FixedBitSet;
use parking_lot::Mutex;

use crate::{
    cli::{BinseqMode, SplitCommand},
    commands::{grep::PatternCollection, match_output, utils::make_directory},
    types::BoxedWriter,
};

#[derive(Clone)]
pub struct AhoCorasickSplitter {
    state1: AhoCorasick,
    state2: AhoCorasick,
    state: AhoCorasick,

    /// bitset over all patterns
    all_bits: FixedBitSet,

    /// bitset over all unique aliases
    unique_bits: FixedBitSet,

    /// unique aliases across all pattern sets
    unique_aliases: Vec<String>,

    /// points to which alias is present at global pattern index
    alias_indices: Vec<usize>,
}

impl AhoCorasickSplitter {
    pub fn new(
        pat1: PatternCollection,
        pat2: PatternCollection,
        pat: PatternCollection,
        no_dfa: bool,
    ) -> Result<Self> {
        let state1 = corasick_builder(&pat1.bytes(), no_dfa)?;
        let state2 = corasick_builder(&pat2.bytes(), no_dfa)?;
        let state = corasick_builder(&pat.bytes(), no_dfa)?;

        let all_bits = FixedBitSet::with_capacity(pat1.len() + pat2.len() + pat.len());

        let mut alias_indices = Vec::new();
        let mut unique_aliases = Vec::new();
        let mut map = HashMap::new();
        for name in pat1
            .names()
            .iter()
            .chain(pat2.names().iter())
            .chain(pat.names().iter())
        {
            let idx = if let Some(idx) = map.get(&name) {
                *idx
            } else {
                unique_aliases.push(name.to_string());
                let alias_index = map.len();
                map.insert(name, alias_index);
                alias_index
            };
            alias_indices.push(idx);
        }
        let unique_bits = FixedBitSet::with_capacity(unique_aliases.len());

        Ok(Self {
            state1,
            state2,
            state,
            all_bits,
            alias_indices,
            unique_aliases,
            unique_bits,
        })
    }

    fn reset_bits(&mut self) {
        self.all_bits.clear();
        self.unique_bits.clear();
    }

    fn match_primary(&mut self, sequence: &[u8]) {
        match_patterns(&self.state1, &mut self.all_bits, sequence, None, 0);
    }

    fn match_secondary(&mut self, sequence: &[u8]) {
        match_patterns(
            &self.state2,
            &mut self.all_bits,
            sequence,
            None,
            self.state1.patterns_len(),
        );
    }

    fn match_either(&mut self, primary: &[u8], secondary: &[u8]) {
        match_patterns(
            &self.state,
            &mut self.all_bits,
            primary,
            Some(secondary),
            self.state1.patterns_len() + self.state2.patterns_len(),
        );
    }

    pub fn split_idx(&mut self, primary: &[u8], secondary: &[u8]) -> Option<usize> {
        self.reset_bits();
        self.match_primary(primary);
        self.match_secondary(secondary);
        self.match_either(primary, secondary);

        self.all_bits.ones().for_each(|idx| {
            self.alias_indices
                .get(idx)
                .map(|u_idx| self.unique_bits.set(*u_idx, true));
        });

        get_single_hit(&self.unique_bits)
    }
}

fn match_patterns(
    patterns: &AhoCorasick,
    bitset: &mut FixedBitSet,
    seq_a: &[u8],
    seq_b: Option<&[u8]>,
    offset: usize,
) {
    if patterns.patterns_len() == 0 {
        return;
    }

    let mut fill_bitset = |seq: &[u8]| {
        if !seq.is_empty() {
            patterns
                .find_overlapping_iter(seq)
                .for_each(|m| bitset.set(offset + m.pattern().as_usize(), true));
        }
    };

    fill_bitset(seq_a);
    seq_b.map(fill_bitset);
}

fn get_single_hit(bitset: &FixedBitSet) -> Option<usize> {
    let mut num_hits = 0;
    let match_id = bitset
        .ones()
        .map(|idx| {
            num_hits += 1;
            idx as usize
        })
        .last();

    if num_hits == 1 {
        match_id
    } else {
        None
    }
}

fn corasick_builder(patterns: &[Vec<u8>], no_dfa: bool) -> Result<AhoCorasick> {
    Ok(AhoCorasickBuilder::new()
        .ascii_case_insensitive(false)
        .kind(if no_dfa {
            None
        } else {
            Some(AhoCorasickKind::DFA)
        })
        .build(patterns)?)
}

#[derive(Clone)]
pub struct SplitProcessor {
    /// Thread-local matcher
    matcher: AhoCorasickSplitter,

    /// Whether the undetermined writer is active
    write_undetermined: bool,

    /// Thread-local writers for the split processor.
    t_writer: Vec<BinseqWriter<Vec<u8>>>,
    t_counts: Vec<usize>,

    /// Global writers for the split processor.
    writer: Arc<Vec<Mutex<BinseqWriter<BoxedWriter>>>>,
    counts: Arc<Vec<Mutex<usize>>>,
}
impl SplitProcessor {
    pub fn new<P: AsRef<Path>>(
        matcher: AhoCorasickSplitter,
        builder: &BinseqWriterBuilder,
        output_basepath: P,
        output_mode: BinseqMode,
        write_undetermined: bool,
        undetermined_basepath: &str,
    ) -> Result<Self> {
        let mut t_writer = Vec::default();
        let mut writer = Vec::default();

        let mut extend_writers = |basename| -> Result<()> {
            let output_path =
                output_basepath
                    .as_ref()
                    .join(format!("{}{}", basename, output_mode.extension()));
            let output_handle = match_output(Some(output_path))?;

            let gw = builder.clone().build(output_handle)?;
            let tw = gw.new_headless_buffer()?;

            t_writer.push(tw);
            writer.push(Mutex::new(gw));

            Ok(())
        };

        matcher
            .unique_aliases
            .iter()
            .map(|s| s.as_str())
            .try_for_each(&mut extend_writers)?;

        if write_undetermined {
            extend_writers(undetermined_basepath)?;
        }

        let t_counts = vec![0; t_writer.len()];
        let counts = Arc::new(Vec::from_iter(
            (0..writer.len()).map(|_| Mutex::new(0)).collect::<Vec<_>>(),
        ));

        Ok(Self {
            matcher,
            write_undetermined,
            t_writer,
            t_counts,
            writer: Arc::new(writer),
            counts,
        })
    }

    fn finish(&mut self) -> binseq::Result<()> {
        self.writer.iter().try_for_each(|w| w.lock().finish())
    }

    fn pprint_counts(&self) -> Result<()> {
        let mut handle = stderr();
        self.matcher
            .unique_aliases
            .iter()
            .zip(self.counts.iter().map(|x| *x.lock()))
            .try_for_each(|(alias, count)| writeln!(&mut handle, "{alias}\t{count}"))?;
        handle.flush().map_err(Into::into)
    }
}
impl ParallelProcessor for SplitProcessor {
    fn process_record<R: binseq::prelude::BinseqRecord>(
        &mut self,
        record: R,
    ) -> binseq::Result<()> {
        let sseq = record.sseq();
        let xseq = record.xseq();
        let rec = if record.is_paired() {
            SequencingRecordBuilder::default()
                .s_seq(record.sseq())
                .opt_s_qual(record.has_quality().then(|| record.squal()))
                .s_header(record.sheader())
                .x_seq(record.xseq())
                .opt_x_qual(record.has_quality().then(|| record.xqual()))
                .x_header(record.xheader())
                .build()?
        } else {
            SequencingRecordBuilder::default()
                .s_seq(record.sseq())
                .opt_s_qual(record.has_quality().then(|| record.squal()))
                .s_header(record.sheader())
                .build()?
        };
        if let Some(pattern_idx) = self.matcher.split_idx(sseq, xseq) {
            // handle match
            self.t_writer
                .get_mut(pattern_idx)
                .map(|w| -> binseq::Result<()> {
                    w.push(rec)?;
                    Ok(())
                });

            self.t_counts.get_mut(pattern_idx).map(|c| {
                *c += 1;
            });
        } else if self.write_undetermined {
            // always the last writer
            let undetermined_idx = self.t_writer.len() - 1;

            // handle match
            self.t_writer
                .get_mut(undetermined_idx)
                .expect("number of writers misconfigured (undetermined)")
                .push(rec)?;

            self.t_counts.get_mut(undetermined_idx).map(|c| {
                *c += 1;
            });
        }
        Ok(())
    }

    fn on_batch_complete(&mut self) -> binseq::Result<()> {
        // ingest counts
        self.counts.iter().zip(self.t_counts.iter_mut()).for_each(
            |(global_counts, thread_counts)| {
                *global_counts.lock() += *thread_counts;
                *thread_counts = 0;
            },
        );

        // ingest reads
        self.writer
            .iter()
            .zip(self.t_writer.iter_mut())
            .try_for_each(|(global_writer, thread_writer)| {
                global_writer.lock().ingest(thread_writer)
            })
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
    let (pat1, pat2, pat) = args.patterns.load_all_patterns()?;
    let splitter = AhoCorasickSplitter::new(pat1, pat2, pat, args.split.no_dfa)?;
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
