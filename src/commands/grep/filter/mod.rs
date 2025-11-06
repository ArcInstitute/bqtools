use std::collections::HashSet;

#[cfg(feature = "fuzzy")]
mod fuzzy_matcher;
mod processor;
mod regex_matcher;

#[cfg(feature = "fuzzy")]
pub use fuzzy_matcher::FuzzyMatcher;
pub use processor::FilterProcessor;
pub use regex_matcher::RegexMatcher;

pub type MatchRanges = HashSet<(usize, usize)>;

pub trait PatternMatcher: Clone + Send + Sync {
    fn match_primary(
        &mut self,
        sequence: &[u8],
        matches: &mut MatchRanges,
        and_logic: bool,
    ) -> bool;
    fn match_secondary(
        &mut self,
        sequence: &[u8],
        matches: &mut MatchRanges,
        and_logic: bool,
    ) -> bool;
    fn match_either(
        &mut self,
        primary: &[u8],
        secondary: &[u8],
        smatches: &mut MatchRanges,
        xmatches: &mut MatchRanges,
        and_logic: bool,
    ) -> bool;
}
