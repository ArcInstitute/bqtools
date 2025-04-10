use std::sync::Arc;

use crate::cli::{BinseqMode, FileFormat, GrepCommand, Mate};
use anyhow::Result;
use memchr::memmem;
use parking_lot::Mutex;

use super::decode::{build_writer, write_record_pair, SplitWriter};

type Pattern = Vec<Vec<u8>>;

#[derive(Clone)]
struct GrepProcessor {
    /// Patterns to search for
    mp1: Pattern, // in primary
    mp2: Pattern, // in secondary
    pat: Pattern, // in either

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
}
impl GrepProcessor {
    pub fn new(
        mp1: Pattern,
        mp2: Pattern,
        pat: Pattern,
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
            format,
            mate,
            is_split: writer.is_split(),
            global_writer: Arc::new(Mutex::new(writer)),
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
        self.mp1
            .iter()
            .all(|pat| memmem::find(&self.sbuf, pat).is_some())
    }

    fn search_secondary(&self) -> bool {
        if self.mp2.is_empty() || self.xbuf.is_empty() {
            return true;
        }
        self.mp2
            .iter()
            .any(|pat| memmem::find(&self.xbuf, pat).is_some())
    }

    fn search_either(&self) -> bool {
        if self.pat.is_empty() {
            return true;
        }
        self.pat.iter().any(|pat| {
            memmem::find(&self.sbuf, pat).is_some() || memmem::find(&self.xbuf, pat).is_some()
        })
    }

    pub fn pattern_match(&self) -> bool {
        self.search_primary() && self.search_secondary() && self.search_either()
    }
}
impl binseq::ParallelProcessor for GrepProcessor {
    fn process_record(&mut self, record: binseq::RefRecord) -> binseq::Result<()> {
        self.clear_buffers();

        // Decode sequences
        record.decode_s(&mut self.sbuf)?;
        if record.paired() {
            record.decode_x(&mut self.xbuf)?;
        }

        if self.pattern_match() {
            // decode index
            let mut ibuf = itoa::Buffer::new();
            let index = ibuf.format(record.id()).as_bytes();

            if self.squal.len() < self.sbuf.len() {
                self.squal.resize(self.sbuf.len(), b'?');
            }
            if self.xqual.len() < self.xbuf.len() {
                self.xqual.resize(self.xbuf.len(), b'?');
            }

            write_record_pair(
                &mut self.left,
                &mut self.right,
                &mut self.mixed,
                self.mate,
                self.is_split,
                index,
                &self.sbuf,
                &self.squal,
                &self.xbuf,
                &self.xqual,
                self.format,
            )?;
        }

        Ok(())
    }

    fn on_batch_complete(&mut self) -> Result<(), binseq::Error> {
        // Lock the mutex to write to the global buffer
        {
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
        Ok(())
    }
}

pub fn run(args: GrepCommand) -> Result<()> {
    args.grep.validate()?;
    match args.input.mode()? {
        BinseqMode::Binseq => {
            let reader = binseq::MmapReader::new(args.input.path())?;
            let writer = build_writer(&args.output, reader.header().xlen > 0)?;
            let format = args.output.format()?;
            let mate = if reader.header().xlen > 0 {
                Some(args.output.mate())
            } else {
                None
            };
            let proc = GrepProcessor::new(
                args.grep.bytes_mp1(),
                args.grep.bytes_mp2(),
                args.grep.bytes_pat(),
                writer,
                format,
                mate,
            );
            reader.process_parallel(proc.clone(), args.output.threads())?;
        }
        BinseqMode::VBinseq => {
            unimplemented!("Not implemented for vbinseq yet")
        }
    }

    Ok(())
}
