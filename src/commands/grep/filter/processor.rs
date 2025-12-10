use crate::{
    cli::{FileFormat, Mate},
    commands::{
        decode::{write_record_pair, SplitWriter},
        grep::{color::write_colored_record_pair, SimpleRange},
    },
};
use binseq::prelude::*;
use parking_lot::Mutex;
use std::sync::Arc;

use super::{MatchRanges, PatternMatch};

#[derive(Clone)]
#[allow(clippy::struct_excessive_bools)]
pub struct FilterProcessor<Pm: PatternMatch> {
    matcher: Pm,

    /// Match logic (true = AND, false = OR)
    and_logic: bool,

    /// Invert the pattern selection
    invert: bool,

    /// Only count the number of matches
    count: bool,

    /// Match within range
    range: Option<SimpleRange>,

    /// Local count
    local_count: usize,

    /// Local primary/extended sequence match indices
    smatches: MatchRanges,
    xmatches: MatchRanges,

    /// Local write buffers
    mixed: Vec<u8>, // General purpose, interleaved or singlets
    left: Vec<u8>, // Used when writing pairs of files (R1/R2)
    right: Vec<u8>,
    interval_buffer: Vec<(usize, usize)>, // reused by colored writer for merging intervals

    /// Quality buffers
    squal: Vec<u8>,
    xqual: Vec<u8>,

    /// Write Options
    format: FileFormat,
    mate: Option<Mate>,
    is_split: bool,
    color: bool,

    /// Global values
    global_writer: Arc<Mutex<SplitWriter>>,
    global_count: Arc<Mutex<usize>>,
}
impl<Pm: PatternMatch> FilterProcessor<Pm> {
    #[allow(clippy::fn_params_excessive_bools)]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        matcher: Pm,
        and_logic: bool,
        invert: bool,
        count: bool,
        range: Option<SimpleRange>,
        writer: SplitWriter,
        format: FileFormat,
        mate: Option<Mate>,
        color: bool,
    ) -> Self {
        Self {
            mixed: Vec::new(),
            left: Vec::new(),
            right: Vec::new(),
            squal: Vec::new(),
            xqual: Vec::new(),
            smatches: MatchRanges::default(),
            xmatches: MatchRanges::default(),
            interval_buffer: Vec::new(),
            matcher,
            and_logic,
            invert,
            count,
            range,
            format,
            mate,
            color,
            is_split: writer.is_split(),
            global_writer: Arc::new(Mutex::new(writer)),
            local_count: 0,
            global_count: Arc::new(Mutex::new(0)),
        }
    }
    pub fn clear_matches(&mut self) {
        self.smatches.clear();
        self.xmatches.clear();
    }

    pub fn pattern_match(&mut self, sbuf: &[u8], xbuf: &[u8]) -> bool {
        let (primary, extended) = if let Some(range) = self.range {
            (range.slice(sbuf), range.slice(xbuf))
        } else {
            (sbuf, xbuf)
        };

        let found_either = self.matcher.match_either(
            primary,
            extended,
            &mut self.smatches,
            &mut self.xmatches,
            self.and_logic,
        );
        let found_primary = self
            .matcher
            .match_primary(primary, &mut self.smatches, self.and_logic);
        let found_secondary =
            self.matcher
                .match_secondary(extended, &mut self.xmatches, self.and_logic);

        let pred = if self.and_logic {
            found_either && found_primary && found_secondary
        } else {
            !self.smatches.is_empty() || !self.xmatches.is_empty()
        };

        if self.invert {
            self.clear_matches(); // ensure no partial matches are highlighted
            !pred
        } else {
            pred
        }
    }
    pub fn pprint_counts(&self) {
        println!("{}", self.global_count.lock());
    }
}

impl<Pm: PatternMatch> ParallelProcessor for FilterProcessor<Pm> {
    fn process_record<B: BinseqRecord>(&mut self, record: B) -> binseq::Result<()> {
        self.clear_matches();

        let sbuf = record.sseq();
        let xbuf = record.xseq();
        if self.pattern_match(sbuf, xbuf) {
            self.local_count += 1;
            if self.count {
                // No further processing needed
                return Ok(());
            }

            let squal = if record.has_quality() {
                record.squal()
            } else {
                if self.squal.len() < sbuf.len() {
                    self.squal.resize(sbuf.len(), b'?');
                }
                &self.squal
            };

            let xqual = if record.is_paired() {
                if record.has_quality() {
                    record.xqual()
                } else {
                    if self.xqual.len() < xbuf.len() {
                        self.xqual.resize(xbuf.len(), b'?');
                    }
                    &self.xqual
                }
            } else {
                if self.xqual.len() < xbuf.len() {
                    self.xqual.resize(xbuf.len(), b'?');
                }
                &self.xqual
            };

            if self.color {
                write_colored_record_pair(
                    &mut self.mixed,
                    self.mate,
                    sbuf,
                    squal,
                    record.sheader(),
                    xbuf,
                    xqual,
                    record.xheader(),
                    &self.smatches,
                    &self.xmatches,
                    self.format,
                    &mut self.interval_buffer,
                )
            } else {
                write_record_pair(
                    &mut self.left,
                    &mut self.right,
                    &mut self.mixed,
                    self.mate,
                    self.is_split,
                    sbuf,
                    squal,
                    record.sheader(),
                    xbuf,
                    xqual,
                    record.xheader(),
                    self.format,
                )
            }?;
        }

        Ok(())
    }

    fn on_batch_complete(&mut self) -> binseq::Result<()> {
        // Lock the mutex to write to the global buffer
        if !self.count {
            let mut writer = self.global_writer.lock();
            if writer.is_split() {
                writer.write_split(&self.left, true)?;
                writer.write_split(&self.right, false)?;
            } else {
                writer.write_interleaved(&self.mixed)?;
            }
            writer.flush()?;
        }

        // Clear the local buffer and reset the local record count
        self.mixed.clear();
        self.left.clear();
        self.right.clear();

        // Increment the global count and reset local
        *self.global_count.lock() += self.local_count;
        self.local_count = 0;

        Ok(())
    }
}
