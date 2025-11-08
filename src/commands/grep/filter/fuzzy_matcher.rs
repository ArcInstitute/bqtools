use super::{MatchRanges, PatternMatch};

use sassy::{profiles::Dna, Searcher};

type Patterns = Vec<Vec<u8>>;

#[derive(Clone)]
pub struct FuzzyMatcher {
    pat1: Patterns,
    pat2: Patterns,
    pat: Patterns,
    k: usize,      // maximum edit distance to accept
    inexact: bool, // whether to only report inexact matches
    offset: usize, // Left-offset relevant for range matching
    searcher: Searcher<Dna>,
}

impl FuzzyMatcher {
    pub fn new(
        pat1: Patterns,
        pat2: Patterns,
        pat: Patterns,
        k: usize,
        inexact: bool,
        offset: usize,
    ) -> Self {
        Self {
            pat1,
            pat2,
            pat,
            k,
            inexact,
            offset,
            searcher: Searcher::new_fwd(),
        }
    }
}

fn find_and_insert_matches(
    pat: &[u8],
    sequence: &[u8],
    matches: &mut MatchRanges,
    searcher: &mut Searcher<Dna>,
    k: usize,
    inexact: bool,
    offset: usize,
) -> bool {
    let mut found = false;
    for mat in searcher.search(pat, &sequence, k) {
        if inexact && mat.cost == 0 {
            continue;
        }
        matches.insert((mat.text_start + offset, mat.text_end + offset));
        found = true;
    }
    found
}

impl PatternMatch for FuzzyMatcher {
    fn offset(&self) -> usize {
        self.offset
    }

    fn match_primary(
        &mut self,
        sequence: &[u8],
        matches: &mut MatchRanges,
        and_logic: bool,
    ) -> bool {
        if self.pat1.is_empty() {
            return true;
        }
        let offset = self.offset();
        let closure = |pat: &Vec<u8>| {
            find_and_insert_matches(
                pat,
                sequence,
                matches,
                &mut self.searcher,
                self.k,
                self.inexact,
                offset,
            )
        };
        if and_logic {
            self.pat1.iter().all(closure)
        } else {
            self.pat1.iter().any(closure)
        }
    }

    fn match_secondary(
        &mut self,
        sequence: &[u8],
        matches: &mut MatchRanges,
        and_logic: bool,
    ) -> bool {
        if self.pat2.is_empty() || sequence.is_empty() {
            return true;
        }
        let offset = self.offset();
        let closure = |pat: &Vec<u8>| {
            find_and_insert_matches(
                pat,
                sequence,
                matches,
                &mut self.searcher,
                self.k,
                self.inexact,
                offset,
            )
        };
        if and_logic {
            self.pat2.iter().all(closure)
        } else {
            self.pat2.iter().any(closure)
        }
    }

    fn match_either(
        &mut self,
        primary: &[u8],
        secondary: &[u8],
        smatches: &mut MatchRanges,
        xmatches: &mut MatchRanges,
        and_logic: bool,
    ) -> bool {
        if self.pat.is_empty() {
            return true;
        }
        let offset = self.offset();
        let closure = |pat: &Vec<u8>| {
            let found_s = find_and_insert_matches(
                pat,
                primary,
                smatches,
                &mut self.searcher,
                self.k,
                self.inexact,
                offset,
            );
            let found_x = find_and_insert_matches(
                pat,
                secondary,
                xmatches,
                &mut self.searcher,
                self.k,
                self.inexact,
                offset,
            );
            found_s || found_x // OR here because we want to match either primary or secondary
        };
        if and_logic {
            self.pat.iter().all(closure)
        } else {
            self.pat.iter().any(closure)
        }
    }
}
