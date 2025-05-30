use std::sync::Arc;

use crate::cli::{FileFormat, GrepCommand, Mate};
use anyhow::Result;
use binseq::prelude::*;
use memchr::memmem::Finder;
use parking_lot::Mutex;

use super::decode::{build_writer, write_record_pair, SplitWriter};

type Patterns = Vec<Finder<'static>>;
type Expressions = Vec<regex::bytes::Regex>;

#[derive(Clone)]
struct GrepProcessor {
    /// Patterns to search for
    mp1: Patterns, // in primary
    mp2: Patterns, // in secondary
    pat: Patterns, // in either

    /// Regex expressions to match on
    re1: Expressions, // in primary
    re2: Expressions, // in secondary
    re: Expressions,  // in either

    /// Invert the pattern selection
    invert: bool,

    /// Only count the number of matches
    count: bool,

    /// Local count
    local_count: usize,

    /// Local write buffers
    mixed: Vec<u8>, // General purpose, interleaved or singlets
    left: Vec<u8>, // Used when writing pairs of files (R1/R2)
    right: Vec<u8>,

    /// Local decoding buffers
    sbuf: Vec<u8>,
    xbuf: Vec<u8>,

    /// Quality buffers
    squal: Vec<u8>,
    xqual: Vec<u8>,

    /// Write Options
    format: FileFormat,
    mate: Option<Mate>,
    is_split: bool,

    /// Global values
    global_writer: Arc<Mutex<SplitWriter>>,
    global_count: Arc<Mutex<usize>>,
}
impl GrepProcessor {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        mp1: Patterns,
        mp2: Patterns,
        pat: Patterns,
        re1: Expressions,
        re2: Expressions,
        re: Expressions,
        invert: bool,
        count: bool,
        writer: SplitWriter,
        format: FileFormat,
        mate: Option<Mate>,
    ) -> Self {
        Self {
            mixed: Vec::new(),
            left: Vec::new(),
            right: Vec::new(),
            sbuf: Vec::new(),
            xbuf: Vec::new(),
            squal: Vec::new(),
            xqual: Vec::new(),
            mp1,
            mp2,
            pat,
            re1,
            re2,
            re,
            invert,
            count,
            format,
            mate,
            is_split: writer.is_split(),
            global_writer: Arc::new(Mutex::new(writer)),
            local_count: 0,
            global_count: Arc::new(Mutex::new(0)),
        }
    }
    pub fn clear_buffers(&mut self) {
        self.sbuf.clear();
        self.xbuf.clear();
    }

    fn search_primary(&self) -> bool {
        if self.mp1.is_empty() {
            return true;
        }
        self.mp1.iter().all(|pat| pat.find(&self.sbuf).is_some())
    }

    fn search_secondary(&self) -> bool {
        if self.mp2.is_empty() || self.xbuf.is_empty() {
            return true;
        }
        self.mp2.iter().any(|pat| pat.find(&self.xbuf).is_some())
    }

    fn search_either(&self) -> bool {
        if self.pat.is_empty() {
            return true;
        }
        self.pat
            .iter()
            .any(|pat| pat.find(&self.sbuf).is_some() || pat.find(&self.xbuf).is_some())
    }

    fn regex_primary(&self) -> bool {
        if self.re1.is_empty() {
            return true;
        }
        self.re1.iter().any(|re| re.find(&self.sbuf).is_some())
    }

    fn regex_secondary(&self) -> bool {
        if self.re2.is_empty() || self.xbuf.is_empty() {
            return true;
        }
        self.re2.iter().any(|re| re.find(&self.xbuf).is_some())
    }

    fn regex_either(&self) -> bool {
        if self.re.is_empty() {
            return true;
        }
        self.re
            .iter()
            .any(|re| re.find(&self.sbuf).is_some() || re.find(&self.xbuf).is_some())
    }

    pub fn pattern_match(&self) -> bool {
        let pred = self.search_primary()
            && self.search_secondary()
            && self.search_either()
            && self.regex_primary()
            && self.regex_secondary()
            && self.regex_either();
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
        if record.is_paired() {
            record.decode_x(&mut self.xbuf)?;
        }

        if self.pattern_match() {
            self.local_count += 1;
            if self.count {
                // No further processing needed
                return Ok(());
            }

            // decode index
            let mut ibuf = itoa::Buffer::new();
            let index = ibuf.format(record.index()).as_bytes();

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

            write_record_pair(
                &mut self.left,
                &mut self.right,
                &mut self.mixed,
                self.mate,
                self.is_split,
                index,
                &self.sbuf,
                squal,
                &self.xbuf,
                xqual,
                self.format,
            )?;
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
    let proc = GrepProcessor::new(
        args.grep.bytes_mp1(),
        args.grep.bytes_mp2(),
        args.grep.bytes_pat(),
        args.grep.bytes_reg1(),
        args.grep.bytes_reg2(),
        args.grep.bytes_reg(),
        args.grep.invert,
        args.grep.count,
        writer,
        format,
        mate,
    );
    reader.process_parallel(proc.clone(), args.output.threads())?;
    if args.grep.count {
        proc.pprint_counts();
    }

    Ok(())
}
