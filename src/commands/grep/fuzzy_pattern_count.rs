use std::sync::Arc;

use anyhow::Result;
use binseq::prelude::*;
use parking_lot::Mutex;
use sassy::{profiles::Dna, Searcher};

type Patterns = Vec<Vec<u8>>;

#[derive(Clone)]
#[allow(clippy::struct_excessive_bools)]
pub struct FuzzyPatternCountProcessor {
    /// Regex expressions to match on
    pat1: Patterns, // in primary
    pat2: Patterns, // in secondary
    pat: Patterns,  // in either
    k: usize,       // maximum edit distance
    inexact: bool,  // only count inexact matches
    invert: bool,   // invert the match

    searcher: Searcher<Dna>,

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
impl FuzzyPatternCountProcessor {
    #[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools)]
    pub fn new(
        pat1: Patterns,
        pat2: Patterns,
        pat: Patterns,
        k: usize,
        inexact: bool,
        invert: bool,
    ) -> Self {
        let local_pattern_count = vec![0; pat1.len() + pat2.len() + pat.len()];
        let global_pattern_count = Arc::new(
            (0..pat1.len() + pat2.len() + pat.len())
                .map(|_| Mutex::new(0))
                .collect(),
        );
        Self {
            sbuf: Vec::new(),
            xbuf: Vec::new(),
            pat1,
            pat2,
            pat,
            k,
            inexact,
            invert,
            searcher: Searcher::new_fwd(),
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

    fn match_primary(&mut self) {
        if self.pat1.is_empty() {
            return;
        }
        self.pat1.iter().enumerate().for_each(|(index, pat)| {
            let mut counted = false;
            self.searcher
                .search(pat, &self.sbuf, self.k)
                .iter()
                .for_each(|mat| {
                    // don't count multiple occurrences in a single line for a single pattern
                    if !counted {
                        // skip exact matches if necessary
                        if self.inexact && mat.cost == 0 {
                            return;
                        }
                        counted = true;
                    }
                });
            if counted != self.invert {
                self.local_pattern_count[index] += 1;
            }
        });
    }

    fn match_secondary(&mut self) {
        if self.pat2.is_empty() || self.xbuf.is_empty() {
            return;
        }
        self.pat2.iter().enumerate().for_each(|(index, pat)| {
            let mut counted = false;
            self.searcher
                .search(pat, &self.xbuf, self.k)
                .iter()
                .for_each(|mat| {
                    // don't count multiple occurrences in a single line for a single pattern
                    if !counted {
                        // skip exact matches if necessary
                        if self.inexact && mat.cost == 0 {
                            return;
                        }
                        counted = true;
                    }
                });
            if counted != self.invert {
                self.local_pattern_count[self.pat1.len() + index] += 1;
            }
        });
    }

    fn match_either(&mut self) {
        if self.pat.is_empty() {
            return;
        }
        self.pat.iter().enumerate().for_each(|(index, pat)| {
            let mut counted = false;
            self.searcher
                .search(pat, &self.sbuf, self.k)
                .iter()
                .for_each(|mat| {
                    // don't count multiple occurrences in a single line for a single pattern
                    if !counted {
                        // skip exact matches if necessary
                        if self.inexact && mat.cost == 0 {
                            return;
                        }
                        counted = true;
                    }
                });

            self.searcher
                .search(pat, &self.xbuf, self.k)
                .iter()
                .for_each(|mat| {
                    // don't count multiple occurrences in a single line for a single pattern
                    if !counted {
                        // skip exact matches if necessary
                        if self.inexact && mat.cost == 0 {
                            return;
                        }
                        counted = true;
                    }
                });

            if counted != self.invert {
                self.local_pattern_count[self.pat1.len() + self.pat2.len() + index] += 1;
            }
        })
    }

    pub fn pattern_match(&mut self) {
        self.match_either();
        self.match_primary();
        self.match_secondary();
    }

    fn total_counts(&self) -> usize {
        self.global_pattern_count
            .iter()
            .map(|count| *count.lock())
            .sum()
    }

    pub fn pprint_pattern_counts(&self) -> Result<()> {
        let total_counts = self.total_counts();
        let total_records = *self.global_total.lock();

        println!("pattern\tcount\tfrac_matched\tfrac_total");
        self.pat1
            .iter()
            .chain(self.pat2.iter())
            .chain(self.pat.iter())
            .zip(self.global_pattern_count.iter())
            .try_for_each(|(re, count)| -> Result<()> {
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
                let pat_str = std::str::from_utf8(&re)?;
                println!("{}\t{}\t{}\t{}", pat_str, count, frac_matched, frac_total);
                Ok(())
            })?;
        Ok(())
    }
}
impl ParallelProcessor for FuzzyPatternCountProcessor {
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
