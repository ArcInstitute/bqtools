use super::{MatchRanges, PatternMatch};

use anyhow::Result;
use fixedbitset::FixedBitSet;
use log::error;
use sassy::{profiles::Iupac, EncodedPatterns, Searcher};

type Profile = Iupac;
type Patterns = Vec<Vec<u8>>;

#[derive(Clone)]
pub struct FuzzyMatcher {
    /// Encoded patterns for the first pattern collection
    pat1: Option<EncodedPatterns<Profile>>,
    /// Encoded patterns for the second pattern collection
    pat2: Option<EncodedPatterns<Profile>>,
    /// Encoded patterns for the shared pattern collection
    pat: Option<EncodedPatterns<Profile>>,

    /// Maximum edit distance to accept
    k: usize,
    /// Whether to only report inexact matches
    inexact: bool,
    /// Left-offset relevant for range matching
    offset: usize,

    /// Fixed-bitset for pat1
    bs1: FixedBitSet,
    /// Fixed-bitset for pat2
    bs2: FixedBitSet,
    /// Fixed-bitset for pat
    bs: FixedBitSet,

    /// Primary sequence pattern searcher
    searcher_1: Searcher<Iupac>,
    /// Secondary sequence pattern searcher
    searcher_2: Searcher<Iupac>,
    /// Shared sequence pattern searcher
    searcher: Searcher<Iupac>,
}

impl FuzzyMatcher {
    pub fn new(
        pat1: Patterns,
        pat2: Patterns,
        pat: Patterns,
        k: usize,
        inexact: bool,
        offset: usize,
    ) -> Result<Self> {
        // initialize a searcher for each pattern collection
        let mut searcher_1 = Searcher::new_fwd();
        let mut searcher_2 = Searcher::new_fwd();
        let mut searcher = Searcher::new_fwd();

        // validate pattern lengths
        validate_single_pattern_length(&pat1)?;
        validate_single_pattern_length(&pat2)?;
        validate_single_pattern_length(&pat)?;

        // encode the patterns for each collection/searcher combination
        let enc_pat1 = (!pat1.is_empty()).then(|| searcher_1.encode_patterns(&pat1));
        let enc_pat2 = (!pat2.is_empty()).then(|| searcher_2.encode_patterns(&pat2));
        let enc_pat = (!pat.is_empty()).then(|| searcher.encode_patterns(&pat));

        // initialize fixed-bitsets for each pattern collection
        let bs1 = FixedBitSet::with_capacity(pat1.len());
        let bs2 = FixedBitSet::with_capacity(pat2.len());
        let bs = FixedBitSet::with_capacity(pat.len());

        Ok(Self {
            pat1: enc_pat1,
            pat2: enc_pat2,
            pat: enc_pat,
            k,
            inexact,
            offset,
            bs1,
            bs2,
            bs,
            searcher_1,
            searcher_2,
            searcher,
        })
    }
}

fn find_and_insert_matches(
    patterns: &EncodedPatterns<Profile>,
    sequence: &[u8],
    matches: &mut MatchRanges,
    searcher: &mut Searcher<Iupac>,
    bitset: &mut FixedBitSet,
    k: usize,
    inexact: bool,
    offset: usize,
) -> bool {
    let mut found = false;
    searcher
        .search_encoded_patterns(patterns, sequence, k)
        .iter()
        .for_each(|m| {
            if inexact && m.cost == 0 {
                return;
            }
            matches.insert((m.text_start + offset, m.text_end + offset));
            bitset.set(m.pattern_idx, true);
            found = true;
        });
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
        if let Some(ref epat) = self.pat1 {
            self.bs1.clear();
            let offset = self.offset();
            let has_any_match = find_and_insert_matches(
                epat,
                sequence,
                matches,
                &mut self.searcher_1,
                &mut self.bs1,
                self.k,
                self.inexact,
                offset,
            );
            if and_logic {
                has_any_match && self.bs1.is_full()
            } else {
                has_any_match
            }
        } else {
            true
        }
    }

    fn match_secondary(
        &mut self,
        sequence: &[u8],
        matches: &mut MatchRanges,
        and_logic: bool,
    ) -> bool {
        if let Some(ref epat) = self.pat2 {
            self.bs2.clear();
            let offset = self.offset();
            let has_any_match = find_and_insert_matches(
                epat,
                sequence,
                matches,
                &mut self.searcher_2,
                &mut self.bs2,
                self.k,
                self.inexact,
                offset,
            );
            if and_logic {
                has_any_match && self.bs2.is_full()
            } else {
                has_any_match
            }
        } else {
            true
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
        if let Some(ref epat) = self.pat {
            self.bs.clear();
            let offset = self.offset();
            let primary_has_any_match = find_and_insert_matches(
                epat,
                primary,
                smatches,
                &mut self.searcher,
                &mut self.bs,
                self.k,
                self.inexact,
                offset,
            );
            let secondary_has_any_match = find_and_insert_matches(
                epat,
                secondary,
                xmatches,
                &mut self.searcher,
                &mut self.bs,
                self.k,
                self.inexact,
                offset,
            );
            let has_any_match = primary_has_any_match || secondary_has_any_match;
            if and_logic {
                has_any_match && self.bs.is_full()
            } else {
                has_any_match
            }
        } else {
            true
        }
    }
}

fn validate_single_pattern_length(patterns: &Patterns) -> Result<()> {
    if patterns.len() < 2 {
        return Ok(());
    }
    let plen = patterns[0].len();
    for pattern in patterns {
        if pattern.len() != plen {
            error!("Multiple pattern lengths provided - currently cannot handle variable-length patterns in fuzzy matching");
            return Err(anyhow::anyhow!("Pattern length mismatch"));
        }
    }

    Ok(())
}
