use super::{MatchRanges, PatternMatch};

type Expressions = Vec<regex::bytes::Regex>;

#[derive(Clone)]
pub struct RegexMatcher {
    /// Regex expressions to match on
    re1: Expressions, // in primary
    re2: Expressions, // in secondary
    re: Expressions,  // in either
    offset: usize,    // left-offset (relevant for range slicing)
}

impl RegexMatcher {
    pub fn new(re1: Expressions, re2: Expressions, re: Expressions, offset: usize) -> Self {
        Self {
            re1,
            re2,
            re,
            offset,
        }
    }
}

fn find_and_insert_matches(
    reg: &regex::bytes::Regex,
    sequence: &[u8],
    matches: &mut MatchRanges,
    offset: usize,
) -> bool {
    let mut found = false;
    for index in reg.find_iter(sequence) {
        matches.insert((index.start() + offset, index.end() + offset));
        found = true;
    }
    found
}

impl PatternMatch for RegexMatcher {
    fn offset(&self) -> usize {
        self.offset
    }

    fn match_primary(
        &mut self,
        sequence: &[u8],
        matches: &mut MatchRanges,
        and_logic: bool,
    ) -> bool {
        if self.re1.is_empty() {
            return true;
        }
        let closure = |reg| find_and_insert_matches(reg, sequence, matches, self.offset());
        if and_logic {
            self.re1.iter().all(closure)
        } else {
            self.re1.iter().any(closure)
        }
    }

    fn match_secondary(
        &mut self,
        sequence: &[u8],
        matches: &mut MatchRanges,
        and_logic: bool,
    ) -> bool {
        if self.re2.is_empty() || sequence.is_empty() {
            return true;
        }
        let closure = |reg| find_and_insert_matches(reg, sequence, matches, self.offset());
        if and_logic {
            self.re2.iter().all(closure)
        } else {
            self.re2.iter().any(closure)
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
        if self.re.is_empty() {
            return true;
        }
        let closure = |reg| {
            let found_s = find_and_insert_matches(reg, primary, smatches, self.offset());
            let found_x = find_and_insert_matches(reg, secondary, xmatches, self.offset());
            found_s || found_x // or because we want to match either
        };
        if and_logic {
            self.re.iter().all(closure)
        } else {
            self.re.iter().any(closure)
        }
    }
}
