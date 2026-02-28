use super::{PatternCollection, PatternCount};

use sassy::{profiles::Iupac, Searcher};

type Patterns = Vec<Vec<u8>>;
#[derive(Clone)]
pub struct FuzzyPatternCounter {
    /// Patterns to fuzzy match on
    pat1: Patterns, // in primary
    pat2: Patterns, // in secondary
    pat: Patterns,  // in either
    k: usize,       // maximum edit distance to accept
    inexact: bool,  // whether to only report inexact matches
    invert: bool,   // invert the match

    all_patterns: PatternCollection,
    searcher: Searcher<Iupac>,
}

impl FuzzyPatternCounter {
    pub fn new(
        pat1: PatternCollection,
        pat2: PatternCollection,
        pat: PatternCollection,
        k: usize,
        inexact: bool,
        invert: bool,
    ) -> Self {
        let pat1_bytes = pat1.bytes();
        let pat2_bytes = pat2.bytes();
        let pat_bytes = pat.bytes();
        let all_patterns = PatternCollection(pat1.into_iter().chain(pat2).chain(pat).collect());
        Self {
            pat1: pat1_bytes,
            pat2: pat2_bytes,
            pat: pat_bytes,
            k,
            inexact,
            invert,
            all_patterns,
            searcher: Searcher::new_fwd(),
        }
    }

    fn match_primary(&mut self, sequence: &[u8], pattern_counts: &mut [usize]) {
        if self.pat1.is_empty() {
            return;
        }
        self.pat1.iter().enumerate().for_each(|(index, pat)| {
            let counted = self
                .searcher
                .search(pat, &sequence, self.k)
                .iter()
                .any(|mat| !self.inexact || mat.cost != 0);
            if counted != self.invert {
                pattern_counts[index] += 1;
            }
        });
    }

    fn match_secondary(&mut self, sequence: &[u8], pattern_counts: &mut [usize]) {
        if self.pat2.is_empty() || sequence.is_empty() {
            return;
        }
        self.pat2.iter().enumerate().for_each(|(index, pat)| {
            let counted = self
                .searcher
                .search(pat, &sequence, self.k)
                .iter()
                .any(|mat| !self.inexact || mat.cost != 0);
            if counted != self.invert {
                pattern_counts[self.pat1.len() + index] += 1;
            }
        });
    }

    fn match_either(&mut self, primary: &[u8], secondary: &[u8], pattern_counts: &mut [usize]) {
        if self.pat.is_empty() {
            return;
        }
        self.pat.iter().enumerate().for_each(|(index, pat)| {
            let counted = self
                .searcher
                .search(pat, &primary, self.k)
                .iter()
                .chain(self.searcher.search(pat, &secondary, self.k).iter())
                .any(|mat| !self.inexact || mat.cost != 0);

            if counted != self.invert {
                pattern_counts[self.pat1.len() + self.pat2.len() + index] += 1;
            }
        });
    }
}

impl PatternCount for FuzzyPatternCounter {
    fn count_patterns(&mut self, primary: &[u8], secondary: &[u8], pattern_count: &mut [usize]) {
        self.match_primary(primary, pattern_count);
        self.match_secondary(secondary, pattern_count);
        self.match_either(primary, secondary, pattern_count);
    }

    fn num_patterns(&self) -> usize {
        self.pat1.len() + self.pat2.len() + self.pat.len()
    }

    fn pattern_strings(&self) -> Vec<String> {
        self.all_patterns
            .iter()
            .map(|pat| {
                std::str::from_utf8(&pat.sequence)
                    .expect("Invalid UTF-8 found in pattern")
                    .to_string()
            })
            .collect()
    }

    fn pattern_names(&self) -> Vec<String> {
        self.all_patterns.names()
    }
}
