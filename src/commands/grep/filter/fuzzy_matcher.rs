use super::{MatchRanges, PatternMatcher};

use sassy::{profiles::Dna, Searcher};

type Patterns = Vec<Vec<u8>>;

#[derive(Clone)]
pub struct FuzzyMatcher {
    pat1: Patterns,
    pat2: Patterns,
    pat: Patterns,
    k: usize,      // maximum edit distance to accept
    inexact: bool, // whether to only report inexact matches
    searcher: Searcher<Dna>,
}

impl FuzzyMatcher {
    pub fn new(pat1: Patterns, pat2: Patterns, pat: Patterns, k: usize, inexact: bool) -> Self {
        Self {
            pat1,
            pat2,
            pat,
            k,
            inexact,
            searcher: Searcher::new_fwd(),
        }
    }
}

impl PatternMatcher for FuzzyMatcher {
    fn match_primary(&mut self, sequence: &[u8], matches: &mut MatchRanges) -> bool {
        if self.pat1.is_empty() {
            return true;
        }
        self.pat1.iter().all(|pat| {
            let mut found = false;
            for mat in self.searcher.search(pat, &sequence, self.k) {
                if self.inexact && mat.cost == 0 {
                    continue;
                }
                matches.insert((mat.text_start, mat.text_end));
                found = true;
            }
            found
        })
    }

    fn match_secondary(&mut self, sequence: &[u8], matches: &mut MatchRanges) -> bool {
        if self.pat2.is_empty() || sequence.is_empty() {
            return true;
        }
        self.pat2.iter().all(|pat| {
            let mut found = false;
            for mat in self.searcher.search(pat, &sequence, self.k) {
                if self.inexact && mat.cost == 0 {
                    continue;
                }
                matches.insert((mat.text_start, mat.text_end));
                found = true;
            }
            found
        })
    }

    fn match_either(
        &mut self,
        primary: &[u8],
        secondary: &[u8],
        smatches: &mut MatchRanges,
        xmatches: &mut MatchRanges,
    ) -> bool {
        if self.pat.is_empty() {
            return true;
        }
        self.pat.iter().all(|pat| {
            let mut found = false;
            for mat in self.searcher.search(pat, &primary, self.k) {
                if self.inexact && mat.cost == 0 {
                    continue;
                }
                smatches.insert((mat.text_start, mat.text_end));
                found = true;
            }
            for mat in self.searcher.search(pat, &secondary, self.k) {
                if self.inexact && mat.cost == 0 {
                    continue;
                }
                xmatches.insert((mat.text_start, mat.text_end));
                found = true;
            }
            found
        })
    }
}
