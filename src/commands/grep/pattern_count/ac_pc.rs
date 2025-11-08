use aho_corasick::AhoCorasick;
use anyhow::Result;
use fixedbitset::FixedBitSet;

use super::PatternCount;

type Patterns = Vec<Vec<u8>>;
#[derive(Clone)]
pub struct AhoCorasickPatternCounter {
    state1: AhoCorasick,
    state2: AhoCorasick,
    state: AhoCorasick,

    bits1: FixedBitSet,
    bits2: FixedBitSet,
    bits: FixedBitSet,

    all_patterns: Patterns,
    invert: bool,
}
impl AhoCorasickPatternCounter {
    pub fn new(pat1: Patterns, pat2: Patterns, pat: Patterns, invert: bool) -> Result<Self> {
        let all_patterns = pat1
            .iter()
            .chain(pat2.iter())
            .chain(pat.iter())
            .cloned()
            .collect();
        Ok(Self {
            state1: AhoCorasick::new(&pat1)?,
            state2: AhoCorasick::new(&pat2)?,
            state: AhoCorasick::new(&pat)?,
            bits1: FixedBitSet::with_capacity(pat1.len()),
            bits2: FixedBitSet::with_capacity(pat2.len()),
            bits: FixedBitSet::with_capacity(pat.len()),
            all_patterns,
            invert,
        })
    }

    fn match_primary(&mut self, sequence: &[u8], pattern_counts: &mut [usize]) {
        if self.state1.patterns_len() == 0 {
            return;
        }

        // set all matched bits
        self.state1.find_overlapping_iter(sequence).for_each(|m| {
            self.bits1.set(m.pattern().as_usize(), true);
        });

        increment_pattern(&self.bits1, pattern_counts, self.invert, 0);
        self.bits1.clear();
    }

    fn match_secondary(&mut self, sequence: &[u8], pattern_counts: &mut [usize]) {
        if self.state2.patterns_len() == 0 || sequence.is_empty() {
            return;
        }

        // set all matched bits
        self.state2.find_overlapping_iter(sequence).for_each(|m| {
            self.bits2.set(m.pattern().as_usize(), true);
        });

        increment_pattern(
            &self.bits2,
            pattern_counts,
            self.invert,
            self.state1.patterns_len(),
        );
        self.bits2.clear();
    }

    fn match_either(&mut self, primary: &[u8], secondary: &[u8], pattern_counts: &mut [usize]) {
        if self.state.patterns_len() == 0 {
            return;
        }

        self.state.find_overlapping_iter(primary).for_each(|m| {
            self.bits.set(m.pattern().as_usize(), true);
        });
        if !secondary.is_empty() {
            self.state.find_overlapping_iter(secondary).for_each(|m| {
                self.bits.set(m.pattern().as_usize(), true);
            });
        }

        increment_pattern(
            &self.bits,
            pattern_counts,
            self.invert,
            self.state1.patterns_len() + self.state2.patterns_len(),
        );
        self.bits.clear();
    }
}

fn increment_pattern(
    bits: &FixedBitSet,
    pattern_counts: &mut [usize],
    invert: bool,
    offset: usize,
) {
    if invert {
        bits.zeroes().for_each(|idx| {
            pattern_counts[offset + idx] += 1;
        });
    } else {
        bits.ones().for_each(|idx| {
            pattern_counts[offset + idx] += 1;
        });
    }
}

impl PatternCount for AhoCorasickPatternCounter {
    fn count_patterns(&mut self, primary: &[u8], secondary: &[u8], pattern_count: &mut [usize]) {
        self.match_primary(primary, pattern_count);
        self.match_secondary(secondary, pattern_count);
        self.match_either(primary, secondary, pattern_count);
    }

    fn num_patterns(&self) -> usize {
        self.state1.patterns_len() + self.state2.patterns_len() + self.state.patterns_len()
    }

    fn pattern_strings(&self) -> Vec<String> {
        self.all_patterns
            .iter()
            .map(|pat| {
                std::str::from_utf8(pat)
                    .expect("Error converting pattern to string")
                    .to_string()
            })
            .collect()
    }
}
