use std::{io::stdout, sync::Arc};

use anyhow::Result;
use binseq::{BinseqRecord, ParallelProcessor};
use parking_lot::Mutex;
use serde::Serialize;

use crate::commands::grep::SimpleRange;

use super::PatternCount;

#[derive(Serialize)]
pub struct PatternCountResult<'a> {
    pattern: &'a str,
    count: usize,
    frac_total: f64,
}
impl<'a> PatternCountResult<'a> {
    pub fn new(pattern: &'a str, count: usize, total: usize) -> Result<Self> {
        Ok(Self {
            pattern,
            count,
            frac_total: if total > 0 {
                count as f64 / total as f64
            } else {
                0.0
            },
        })
    }
}

#[derive(Clone)]
pub struct PatternCountProcessor<Pc: PatternCount> {
    counter: Pc,
    range: Option<SimpleRange>,

    local_pattern_count: Vec<usize>,
    local_total: usize, // total number of reads processed (not just matches)

    /// Global values
    global_pattern_count: Arc<Vec<Mutex<usize>>>,
    global_total: Arc<Mutex<usize>>, // total number of reads processed
}
impl<Pc: PatternCount> PatternCountProcessor<Pc> {
    pub fn new(counter: Pc, range: Option<SimpleRange>) -> Self {
        let num_patterns = counter.num_patterns();
        Self {
            counter,
            range,
            local_pattern_count: vec![0; num_patterns],
            local_total: 0,
            global_pattern_count: Arc::new((0..num_patterns).map(|_| Mutex::new(0)).collect()),
            global_total: Arc::new(Mutex::new(0)),
        }
    }
    pub fn pprint_pattern_counts(&self) -> Result<()> {
        let mut writer = csv::WriterBuilder::new()
            .delimiter(b'\t')
            .has_headers(true)
            .from_writer(stdout());

        let total_records = *self.global_total.lock();
        let patterns = self.counter.pattern_strings();

        patterns
            .iter()
            .zip(self.global_pattern_count.iter())
            .try_for_each(|(pattern, count)| -> Result<()> {
                let record = PatternCountResult::new(pattern, *count.lock(), total_records)?;
                writer.serialize(record)?;
                Ok(())
            })?;

        writer.flush()?;
        Ok(())
    }
}
impl<Pc: PatternCount> ParallelProcessor for PatternCountProcessor<Pc> {
    fn process_record<B: BinseqRecord>(&mut self, record: B) -> binseq::Result<()> {
        // grab sequences
        let sbuf = record.sseq();
        let xbuf = record.xseq();

        let (primary, extended) = if let Some(range) = self.range {
            (range.slice(sbuf), range.slice(xbuf))
        } else {
            (sbuf, xbuf)
        };

        self.counter
            .count_patterns(primary, extended, &mut self.local_pattern_count);
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
