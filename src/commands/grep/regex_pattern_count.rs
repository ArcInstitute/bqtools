use std::sync::Arc;

use binseq::prelude::*;
use parking_lot::Mutex;

type Expressions = Vec<regex::bytes::Regex>;

#[derive(Clone)]
#[allow(clippy::struct_excessive_bools)]
pub struct GrepPatternCountProcessor {
    /// Regex expressions to match on
    re1: Expressions, // in primary
    re2: Expressions, // in secondary
    re: Expressions,  // in either
    invert: bool,

    /// Local count
    local_pattern_count: Vec<usize>,
    local_total: usize, // total number of reads processed (not just matches)

    /// Local decoding buffers
    sbuf: Vec<u8>,
    xbuf: Vec<u8>,

    /// Global values
    global_pattern_count: Arc<Vec<Mutex<usize>>>,
    global_total: Arc<Mutex<usize>>, // total number of reads processed
}
impl GrepPatternCountProcessor {
    #[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools)]
    pub fn new(re1: Expressions, re2: Expressions, re: Expressions, invert: bool) -> Self {
        let local_pattern_count = vec![0; re1.len() + re2.len() + re.len()];
        let global_pattern_count = Arc::new(
            (0..re1.len() + re2.len() + re.len())
                .map(|_| Mutex::new(0))
                .collect(),
        );
        Self {
            sbuf: Vec::new(),
            xbuf: Vec::new(),
            re1,
            re2,
            re,
            invert,
            local_total: 0,
            local_pattern_count,
            global_pattern_count,
            global_total: Arc::new(Mutex::new(0)),
        }
    }
    pub fn clear_buffers(&mut self) {
        self.sbuf.clear();
        self.xbuf.clear();
    }

    fn regex_primary(&mut self) {
        if self.re1.is_empty() {
            return;
        }
        self.re1.iter().enumerate().for_each(|(index, reg)| {
            if reg.find(&self.sbuf).is_some() {
                if !self.invert {
                    self.local_pattern_count[index] += 1;
                }
            } else if self.invert {
                self.local_pattern_count[index] += 1;
            }
        });
    }

    fn regex_secondary(&mut self) {
        if self.re2.is_empty() || self.xbuf.is_empty() {
            return;
        }
        self.re2.iter().enumerate().for_each(|(index, reg)| {
            if reg.find(&self.xbuf).is_some() {
                if !self.invert {
                    self.local_pattern_count[self.re1.len() + index] += 1;
                }
            } else if self.invert {
                self.local_pattern_count[self.re1.len() + index] += 1;
            }
        });
    }

    fn regex_either(&mut self) {
        if self.re.is_empty() {
            return;
        }
        self.re.iter().enumerate().for_each(|(index, reg)| {
            if reg.find(&self.sbuf).is_some() || reg.find(&self.xbuf).is_some() {
                if !self.invert {
                    self.local_pattern_count[self.re1.len() + self.re2.len() + index] += 1;
                }
            } else if self.invert {
                self.local_pattern_count[self.re1.len() + self.re2.len() + index] += 1;
            }
        });
    }

    pub fn pattern_match(&mut self) {
        self.regex_either();
        self.regex_primary();
        self.regex_secondary();
    }

    fn total_counts(&self) -> usize {
        self.global_pattern_count
            .iter()
            .map(|count| *count.lock())
            .sum()
    }

    pub fn pprint_pattern_counts(&self) {
        let total_counts = self.total_counts();
        let total_records = *self.global_total.lock();

        println!("pattern\tcount\tfrac_matched\tfrac_total");
        self.re1
            .iter()
            .chain(self.re2.iter())
            .chain(self.re.iter())
            .zip(self.global_pattern_count.iter())
            .for_each(|(re, count)| {
                let count = *count.lock();
                let frac_matched = if total_counts > 0 {
                    count as f64 / total_counts as f64
                } else {
                    0.0
                };
                let frac_total = if total_records > 0 {
                    count as f64 / total_records as f64
                } else {
                    0.0
                };
                println!("{re}\t{count}\t{frac_matched}\t{frac_total}");
            });
    }
}
impl ParallelProcessor for GrepPatternCountProcessor {
    fn process_record<B: BinseqRecord>(&mut self, record: B) -> binseq::Result<()> {
        self.clear_buffers();
        // Decode sequences
        record.decode_s(&mut self.sbuf)?;
        if record.is_paired() {
            record.decode_x(&mut self.xbuf)?;
        }
        self.pattern_match();
        self.local_total += 1;
        Ok(())
    }

    fn on_batch_complete(&mut self) -> binseq::Result<()> {
        // update the local and global pattern counts
        self.local_pattern_count
            .iter_mut()
            .zip(self.global_pattern_count.iter())
            .for_each(|(local, global)| {
                *global.lock() += *local;
                *local = 0;
            });

        // update the local and global total records processed
        {
            *self.global_total.lock() += self.local_total;
            self.local_total = 0;
        }

        Ok(())
    }
}
