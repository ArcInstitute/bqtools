use super::{PatternCollection, PatternCount};

use fixedbitset::FixedBitSet;
use sassy::{profiles::Iupac, EncodedPatterns, Match, Searcher};

type Profile = Iupac;

#[derive(Clone)]
pub struct FuzzyPatternCounter {
    /// Patterns to fuzzy match on
    pat1: Option<EncodedPatterns<Profile>>, // in primary
    pat2: Option<EncodedPatterns<Profile>>, // in secondary
    pat: Option<EncodedPatterns<Profile>>,  // in either
    k: usize,                               // maximum edit distance to accept
    inexact: bool,                          // whether to only report inexact matches
    invert: bool,                           // invert the match

    /// Fixed bitset for pat1
    bs1: FixedBitSet,
    /// Fixed bitset for pat2
    bs2: FixedBitSet,
    /// Fixed bitset for pat
    bs: FixedBitSet,

    all_patterns: PatternCollection,

    /// Primary sequence searcher
    searcher_1: Searcher<Profile>,
    /// Extended sequence searcher
    searcher_2: Searcher<Profile>,
    /// Combined searcher
    searcher: Searcher<Profile>,
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
        // initialize a searcher for each pattern collection
        let mut searcher_1 = Searcher::new_fwd();
        let mut searcher_2 = Searcher::new_fwd();
        let mut searcher = Searcher::new_fwd();

        // encode the patterns for each collection/searcher combination
        let enc_pat1 = pat1
            .has_patterns()
            .then(|| searcher_1.encode_patterns(&pat1.bytes()));
        let enc_pat2 = pat2
            .has_patterns()
            .then(|| searcher_2.encode_patterns(&pat2.bytes()));
        let enc_pat = pat
            .has_patterns()
            .then(|| searcher.encode_patterns(&pat.bytes()));

        let bs1 = FixedBitSet::with_capacity(pat1.len());
        let bs2 = FixedBitSet::with_capacity(pat2.len());
        let bs = FixedBitSet::with_capacity(pat.len());

        // combine all patterns into a single collection for reporting
        let all_patterns = PatternCollection(pat1.into_iter().chain(pat2).chain(pat).collect());

        Self {
            pat1: enc_pat1,
            pat2: enc_pat2,
            pat: enc_pat,
            k,
            inexact,
            invert,
            all_patterns,
            bs1,
            bs2,
            bs,
            searcher_1,
            searcher_2,
            searcher,
        }
    }

    fn match_primary(&mut self, sequence: &[u8]) {
        if let Some(ref epat) = self.pat1 {
            self.searcher_1
                .search_encoded_patterns(epat, sequence, self.k)
                .iter()
                .for_each(|m| {
                    let counted = !self.inexact || m.cost != 0;
                    if counted {
                        self.bs1.set(m.pattern_idx, true);
                    }
                });
        }
    }

    fn match_secondary(&mut self, sequence: &[u8]) {
        if let Some(ref epat) = self.pat2 {
            self.searcher_2
                .search_encoded_patterns(epat, sequence, self.k)
                .iter()
                .for_each(|m| {
                    let counted = !self.inexact || m.cost != 0;
                    if counted {
                        self.bs2.set(m.pattern_idx, true);
                    }
                });
        }
    }

    fn match_either(&mut self, primary: &[u8], secondary: &[u8]) {
        if let Some(ref epat) = self.pat {
            let mut eval = |m: &Match| {
                let counted = !self.inexact || m.cost != 0;
                if counted {
                    self.bs.set(m.pattern_idx, true);
                }
            };

            // match on primary
            self.searcher
                .search_encoded_patterns(epat, primary, self.k)
                .iter()
                .for_each(|m| eval(m));

            // match on secondary
            self.searcher
                .search_encoded_patterns(epat, secondary, self.k)
                .iter()
                .for_each(|m| eval(m));
        }
    }

    fn clear_bits(&mut self) {
        self.bs1.clear();
        self.bs2.clear();
        self.bs.clear();
    }

    fn update_pattern_count(&mut self, pattern_count: &mut [usize]) {
        // evaluate the bitset for each pattern type
        let mut eval = |bitset: &FixedBitSet, invert: bool, offset: usize| {
            if invert {
                bitset.zeroes().for_each(|i| {
                    pattern_count[i as usize + offset] += 1;
                });
            } else {
                bitset.ones().for_each(|i| {
                    pattern_count[i as usize + offset] += 1;
                });
            }
        };

        eval(&self.bs1, self.invert, 0);
        eval(&self.bs2, self.invert, self.bs1.len());
        eval(&self.bs, self.invert, self.bs1.len() + self.bs2.len());
    }
}

impl PatternCount for FuzzyPatternCounter {
    fn count_patterns(&mut self, primary: &[u8], secondary: &[u8], pattern_count: &mut [usize]) {
        // remove all previous matches
        self.clear_bits();

        // match patterns
        self.match_primary(primary);
        self.match_secondary(secondary);
        self.match_either(primary, secondary);

        // update pattern count (invert if necessary)
        self.update_pattern_count(pattern_count);
    }

    fn num_patterns(&self) -> usize {
        self.pat1.as_ref().map_or(0, |p| p.n_queries())
            + self.pat2.as_ref().map_or(0, |p| p.n_queries())
            + self.pat.as_ref().map_or(0, |p| p.n_queries())
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
