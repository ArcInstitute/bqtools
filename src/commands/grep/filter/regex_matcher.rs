use super::{MatchRanges, PatternMatcher};

type Expressions = Vec<regex::bytes::Regex>;

#[derive(Clone)]
pub struct RegexMatcher {
    /// Regex expressions to match on
    re1: Expressions, // in primary
    re2: Expressions, // in secondary
    re: Expressions,  // in either
}

impl RegexMatcher {
    pub fn new(re1: Expressions, re2: Expressions, re: Expressions) -> Self {
        Self { re1, re2, re }
    }
}

impl PatternMatcher for RegexMatcher {
    fn match_primary(&mut self, sequence: &[u8], matches: &mut MatchRanges) -> bool {
        if self.re1.is_empty() {
            return true;
        }
        self.re1.iter().all(|reg| {
            let mut found = false;
            for index in reg.find_iter(sequence) {
                matches.insert((index.start(), index.end()));
                found = true;
            }
            found
        })
    }

    fn match_secondary(&mut self, sequence: &[u8], matches: &mut MatchRanges) -> bool {
        if self.re2.is_empty() || sequence.is_empty() {
            return true;
        }
        self.re2.iter().all(|reg| {
            let mut found = false;
            for index in reg.find_iter(sequence) {
                matches.insert((index.start(), index.end()));
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
        if self.re.is_empty() {
            return true;
        }
        self.re.iter().all(|reg| {
            let mut found = false;
            for index in reg.find_iter(primary) {
                smatches.insert((index.start(), index.end()));
                found = true;
            }
            for index in reg.find_iter(secondary) {
                xmatches.insert((index.start(), index.end()));
                found = true;
            }
            found
        })
    }
}
