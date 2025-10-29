use std::{collections::HashSet, sync::Arc};

use super::color::write_colored_record_pair;
use crate::{
    cli::{FileFormat, Mate},
    commands::decode::{write_record_pair, SplitWriter},
};

use binseq::prelude::*;
use parking_lot::Mutex;
use sassy::{profiles::Dna, Searcher};

type Patterns = Vec<Vec<u8>>;

#[allow(clippy::struct_excessive_bools)]
pub struct GrepProcessor {
    /// Patterns to fuzzy match on
    pat1: Patterns, // in primary
    pat2: Patterns, // in secondary
    pat: Patterns,  // in either
    k: usize,       // maximum edit distance to accept

    searcher: Searcher<Dna>,

    /// Match logic (true = AND, false = OR)
    and_logic: bool,

    /// Invert the pattern selection
    invert: bool,

    /// Only count the number of matches
    count: bool,

    /// Local count
    local_count: usize,

    /// Local primary/extended sequence match indices
    smatches: HashSet<(usize, usize)>,
    xmatches: HashSet<(usize, usize)>,

    /// Local write buffers
    mixed: Vec<u8>, // General purpose, interleaved or singlets
    left: Vec<u8>, // Used when writing pairs of files (R1/R2)
    right: Vec<u8>,
    interval_buffer: Vec<(usize, usize)>, // reused by colored writer for merging intervals

    /// Local decoding buffers
    sbuf: Vec<u8>,
    xbuf: Vec<u8>,

    /// Quality buffers
    squal: Vec<u8>,
    xqual: Vec<u8>,

    /// Header buffers
    sheader: Vec<u8>,
    xheader: Vec<u8>,

    /// Write Options
    format: FileFormat,
    mate: Option<Mate>,
    is_split: bool,
    color: bool,

    /// Global values
    global_writer: Arc<Mutex<SplitWriter>>,
    global_count: Arc<Mutex<usize>>,
}

impl Clone for GrepProcessor {
    fn clone(&self) -> Self {
        Self {
            pat1: self.pat1.clone(),
            pat2: self.pat2.clone(),
            pat: self.pat.clone(),
            k: self.k,
            searcher: Searcher::new_fwd(), // Initialize searcher with default settings
            and_logic: self.and_logic,
            invert: self.invert,
            count: self.count,
            global_writer: self.global_writer.clone(),
            format: self.format,
            mate: self.mate.clone(),
            color: self.color,
            mixed: Vec::new(),
            left: Vec::new(),
            right: Vec::new(),
            sbuf: Vec::new(),
            xbuf: Vec::new(),
            squal: Vec::new(),
            xqual: Vec::new(),
            sheader: Vec::new(),
            xheader: Vec::new(),
            smatches: HashSet::new(),
            xmatches: HashSet::new(),
            interval_buffer: Vec::new(),
            local_count: 0,
            global_count: Arc::new(Mutex::new(0)),
            is_split: self.is_split,
        }
    }
}

impl GrepProcessor {
    #[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools)]
    pub fn new(
        pat1: Patterns,
        pat2: Patterns,
        pat: Patterns,
        k: usize,
        and_logic: bool,
        invert: bool,
        count: bool,
        writer: SplitWriter,
        format: FileFormat,
        mate: Option<Mate>,
        color: bool,
    ) -> Self {
        Self {
            mixed: Vec::new(),
            left: Vec::new(),
            right: Vec::new(),
            sbuf: Vec::new(),
            xbuf: Vec::new(),
            squal: Vec::new(),
            xqual: Vec::new(),
            sheader: Vec::new(),
            xheader: Vec::new(),
            smatches: HashSet::new(),
            xmatches: HashSet::new(),
            interval_buffer: Vec::new(),
            searcher: Searcher::new_fwd(),
            pat1,
            pat2,
            pat,
            k,
            and_logic,
            invert,
            count,
            format,
            mate,
            color,
            is_split: writer.is_split(),
            global_writer: Arc::new(Mutex::new(writer)),
            local_count: 0,
            global_count: Arc::new(Mutex::new(0)),
        }
    }
    pub fn clear_buffers(&mut self) {
        self.sbuf.clear();
        self.xbuf.clear();
        self.smatches.clear();
        self.xmatches.clear();
    }

    fn regex_primary(&mut self) -> bool {
        if self.pat1.is_empty() {
            return true;
        }
        self.pat1.iter().all(|pat| {
            let mut found = false;
            for mat in self.searcher.search_all(pat, &self.sbuf, self.k) {
                self.smatches.insert((mat.text_start, mat.text_end));
                found = true;
            }
            found
        })
    }

    fn regex_secondary(&mut self) -> bool {
        if self.pat2.is_empty() || self.xbuf.is_empty() {
            return true;
        }
        self.pat2.iter().all(|pat| {
            let mut found = false;
            for mat in self.searcher.search_all(pat, &self.xbuf, self.k) {
                self.xmatches.insert((mat.text_start, mat.text_end));
                found = true;
            }
            found
        })
    }

    fn regex_either(&mut self) -> bool {
        if self.pat.is_empty() {
            return true;
        }
        self.pat.iter().all(|pat| {
            let mut found = false;
            for mat in self.searcher.search_all(pat, &self.sbuf, self.k) {
                self.smatches.insert((mat.text_start, mat.text_end));
                found = true;
            }
            for mat in self.searcher.search_all(pat, &self.xbuf, self.k) {
                self.xmatches.insert((mat.text_start, mat.text_end));
                found = true;
            }
            found
        })
    }

    pub fn pattern_match(&mut self) -> bool {
        let found_either = self.regex_either();
        let found_primary = self.regex_primary();
        let found_secondary = self.regex_secondary();

        let pred = if self.and_logic {
            found_either && found_primary && found_secondary
        } else {
            !self.smatches.is_empty() || !self.xmatches.is_empty()
        };

        if self.invert {
            !pred
        } else {
            pred
        }
    }

    pub fn pprint_counts(&self) {
        println!("{}", self.global_count.lock());
    }
}
impl ParallelProcessor for GrepProcessor {
    fn process_record<B: BinseqRecord>(&mut self, record: B) -> binseq::Result<()> {
        self.clear_buffers();

        // Decode sequences
        record.decode_s(&mut self.sbuf)?;
        record.sheader(&mut self.sheader);
        if record.is_paired() {
            record.decode_x(&mut self.xbuf)?;
            record.xheader(&mut self.xheader);
        }

        if self.pattern_match() {
            self.local_count += 1;
            if self.count {
                // No further processing needed
                return Ok(());
            }

            let squal = if record.has_quality() {
                record.squal()
            } else {
                if self.squal.len() < self.sbuf.len() {
                    self.squal.resize(self.sbuf.len(), b'?');
                }
                &self.squal
            };

            let xqual = if record.is_paired() {
                if record.has_quality() {
                    record.xqual()
                } else {
                    if self.xqual.len() < self.xbuf.len() {
                        self.xqual.resize(self.xbuf.len(), b'?');
                    }
                    &self.xqual
                }
            } else {
                if self.xqual.len() < self.xbuf.len() {
                    self.xqual.resize(self.xbuf.len(), b'?');
                }
                &self.xqual
            };

            if self.color {
                write_colored_record_pair(
                    &mut self.mixed,
                    self.mate,
                    &self.sbuf,
                    squal,
                    &self.sheader,
                    &self.xbuf,
                    xqual,
                    &self.xheader,
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
                    &self.sbuf,
                    squal,
                    &self.sheader,
                    &self.xbuf,
                    xqual,
                    &self.xheader,
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
