use std::{collections::HashSet, sync::Arc};

use super::color::write_colored_record_pair;
use crate::{
    cli::{FileFormat, Mate},
    commands::decode::{write_record_pair, SplitWriter},
};

use binseq::prelude::*;
use parking_lot::Mutex;

type Expressions = Vec<regex::bytes::Regex>;

#[derive(Clone)]
#[allow(clippy::struct_excessive_bools)]
pub struct GrepProcessor {
    /// Regex expressions to match on
    re1: Expressions, // in primary
    re2: Expressions, // in secondary
    re: Expressions,  // in either

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
impl GrepProcessor {
    #[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools)]
    pub fn new(
        re1: Expressions,
        re2: Expressions,
        re: Expressions,
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
            re1,
            re2,
            re,
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
        if self.re1.is_empty() {
            return true;
        }
        self.re1.iter().all(|reg| {
            let mut found = false;
            for index in reg.find_iter(&self.sbuf) {
                self.smatches.insert((index.start(), index.end()));
                found = true;
            }
            found
        })
    }

    fn regex_secondary(&mut self) -> bool {
        if self.re2.is_empty() || self.xbuf.is_empty() {
            return true;
        }
        self.re2.iter().all(|reg| {
            let mut found = false;
            for index in reg.find_iter(&self.xbuf) {
                self.xmatches.insert((index.start(), index.end()));
                found = true;
            }
            found
        })
    }

    fn regex_either(&mut self) -> bool {
        if self.re.is_empty() {
            return true;
        }
        self.re.iter().all(|reg| {
            let mut found = false;
            for index in reg.find_iter(&self.sbuf) {
                self.smatches.insert((index.start(), index.end()));
                found = true;
            }
            for index in reg.find_iter(&self.xbuf) {
                self.xmatches.insert((index.start(), index.end()));
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
