use super::{PatternCollection, PatternCount};

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
            .then_some(searcher_1.encode_patterns(&pat1.bytes()));
        let enc_pat2 = pat2
            .has_patterns()
            .then_some(searcher_2.encode_patterns(&pat2.bytes()));
        let enc_pat = pat
            .has_patterns()
            .then_some(searcher.encode_patterns(&pat.bytes()));

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
            searcher_1,
            searcher_2,
            searcher,
        }
    }

    fn match_primary(&mut self, sequence: &[u8], pattern_counts: &mut [usize]) {
        if let Some(ref epat) = self.pat1 {
            self.searcher_1
                .search_all_encoded_patterns(epat, sequence, self.k)
                .iter()
                .for_each(|m| {
                    let counted = !self.inexact || m.cost != 0;
                    if counted != self.invert {
                        pattern_counts[m.pattern_idx] += 1;
                    }
                });
        }
    }

    fn match_secondary(&mut self, sequence: &[u8], pattern_counts: &mut [usize]) {
        if let Some(ref epat) = self.pat2 {
            let offset = self.pat1.as_ref().map_or(0, |p| p.n_queries());
            self.searcher_2
                .search_all_encoded_patterns(epat, sequence, self.k)
                .iter()
                .for_each(|m| {
                    let counted = !self.inexact || m.cost != 0;
                    if counted != self.invert {
                        pattern_counts[offset + m.pattern_idx] += 1;
                    }
                });
        }
    }

    fn match_either(&mut self, primary: &[u8], secondary: &[u8], pattern_counts: &mut [usize]) {
        if let Some(ref epat) = self.pat {
            let offset = self.pat1.as_ref().map_or(0, |p| p.n_queries())
                + self.pat2.as_ref().map_or(0, |p| p.n_queries());

            let eval = |m: &Match, pattern_counts: &mut [usize]| {
                let counted = !self.inexact || m.cost != 0;
                if counted != self.invert {
                    pattern_counts[offset + m.pattern_idx] += 1;
                }
            };

            // match on primary
            self.searcher
                .search_all_encoded_patterns(epat, primary, self.k)
                .iter()
                .for_each(|m| eval(m, pattern_counts));

            // match on secondary
            self.searcher
                .search_all_encoded_patterns(epat, secondary, self.k)
                .iter()
                .for_each(|m| eval(m, pattern_counts));
        }
    }
}

impl PatternCount for FuzzyPatternCounter {
    fn count_patterns(&mut self, primary: &[u8], secondary: &[u8], pattern_count: &mut [usize]) {
        self.match_primary(primary, pattern_count);
        self.match_secondary(secondary, pattern_count);
        self.match_either(primary, secondary, pattern_count);
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
