use aho_corasick::{AhoCorasick, AhoCorasickBuilder, AhoCorasickKind};
use anyhow::Result;

use crate::commands::grep::filter::{MatchRanges, PatternMatch};

type Patterns = Vec<Vec<u8>>;
#[derive(Clone)]
pub struct AhoCorasickMatcher {
    pat1: AhoCorasick,
    pat2: AhoCorasick,
    pat: AhoCorasick,
    offset: usize,
}

impl AhoCorasickMatcher {
    pub fn new(
        pat1: Patterns,
        pat2: Patterns,
        pat: Patterns,
        no_dfa: bool,
        offset: usize,
    ) -> Result<Self> {
        Ok(Self {
            pat1: corasick_builder(&pat1, no_dfa)?,
            pat2: corasick_builder(&pat2, no_dfa)?,
            pat: corasick_builder(&pat, no_dfa)?,
            offset,
        })
    }
}

fn corasick_builder(patterns: &Patterns, no_dfa: bool) -> Result<AhoCorasick> {
    Ok(AhoCorasickBuilder::new()
        .ascii_case_insensitive(false)
        .kind(if no_dfa {
            None
        } else {
            Some(AhoCorasickKind::DFA)
        })
        .build(patterns)?)
}

fn find_and_insert_matches(
    state: &mut AhoCorasick,
    sequence: &[u8],
    matches: &mut MatchRanges,
    offset: usize,
) -> bool {
    state
        .find_overlapping_iter(sequence)
        .map(|mat| matches.insert((offset + mat.start(), offset + mat.end())))
        .count()
        > 0
}

impl PatternMatch for AhoCorasickMatcher {
    fn offset(&self) -> usize {
        self.offset
    }

    fn match_primary(
        &mut self,
        sequence: &[u8],
        matches: &mut MatchRanges,
        and_logic: bool,
    ) -> bool {
        if self.pat1.patterns_len() == 0 {
            return true;
        }
        if and_logic {
            unimplemented!("AND logic is not supported for Aho-Corasick")
        } else {
            find_and_insert_matches(&mut self.pat1, sequence, matches, self.offset)
        }
    }

    fn match_secondary(
        &mut self,
        sequence: &[u8],
        matches: &mut MatchRanges,
        and_logic: bool,
    ) -> bool {
        if self.pat2.patterns_len() == 0 || sequence.is_empty() {
            return true;
        }
        if and_logic {
            unimplemented!("AND logic is not supported for Aho-Corasick")
        } else {
            find_and_insert_matches(&mut self.pat2, sequence, matches, self.offset)
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
        if self.pat.patterns_len() == 0 {
            return true;
        }
        if and_logic {
            unimplemented!("AND logic is not supported for Aho-Corasick")
        } else {
            find_and_insert_matches(&mut self.pat, primary, smatches, self.offset)
                || find_and_insert_matches(&mut self.pat, secondary, xmatches, self.offset)
        }
    }
}
