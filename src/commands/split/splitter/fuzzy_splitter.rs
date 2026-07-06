use hashbrown::HashMap;

use anyhow::Result;
use fixedbitset::FixedBitSet;
use sassy::{profiles::Iupac, EncodedPatterns, Match, Searcher};

use crate::commands::{
    grep::PatternCollection, split::splitter::SequenceSplit, utils::build_fuzzy_searcher,
};

type Profile = Iupac;

/// Splits records into output bins using fuzzy (edit-distance) matching via `sassy`.
///
/// Patterns are matched against the primary sequence (`pat1`), the secondary
/// sequence (`pat2`), and either sequence (`pat`). A record is assigned to a bin
/// only when its matches resolve to exactly one unique alias.
#[derive(Clone)]
pub struct FuzzySplitter {
    pat1: Option<EncodedPatterns<Profile>>,
    pat2: Option<EncodedPatterns<Profile>>,
    pat: Option<EncodedPatterns<Profile>>,

    /// Number of patterns in `pat1` (offset for `pat2` bits)
    n_pat1: usize,
    /// Number of patterns in `pat2` (offset for `pat` bits)
    n_pat2: usize,

    /// Maximum edit distance to accept
    k: usize,
    /// Whether to only accept inexact matches
    inexact: bool,

    /// Primary sequence searcher
    searcher_1: Searcher<Profile>,
    /// Secondary sequence searcher
    searcher_2: Searcher<Profile>,
    /// Shared sequence searcher
    searcher: Searcher<Profile>,

    /// bitset over all patterns
    all_bits: FixedBitSet,

    /// bitset over all unique aliases
    unique_bits: FixedBitSet,

    /// unique aliases across all pattern sets
    unique_aliases: Vec<String>,

    /// points to which alias is present at global pattern index
    alias_indices: Vec<usize>,
}

impl FuzzySplitter {
    pub fn new(
        pat1: &PatternCollection,
        pat2: &PatternCollection,
        pat: &PatternCollection,
        k: usize,
        inexact: bool,
        max_n_frac: Option<f32>,
    ) -> Result<Self> {
        // validate lengths, resolve max_n_frac, and encode patterns per pattern set
        let (searcher_1, enc_pat1) = build_fuzzy_searcher(&pat1.bytes(), k, max_n_frac)?;
        let (searcher_2, enc_pat2) = build_fuzzy_searcher(&pat2.bytes(), k, max_n_frac)?;
        let (searcher, enc_pat) = build_fuzzy_searcher(&pat.bytes(), k, max_n_frac)?;

        let all_bits = FixedBitSet::with_capacity(pat1.len() + pat2.len() + pat.len());

        let mut alias_indices = Vec::new();
        let mut unique_aliases = Vec::new();
        let mut map = HashMap::new();
        for name in pat1
            .names()
            .into_iter()
            .chain(pat2.names())
            .chain(pat.names())
        {
            let idx = if let Some(idx) = map.get(&name) {
                *idx
            } else {
                unique_aliases.push(name.clone());
                let alias_index = map.len();
                map.insert(name, alias_index);
                alias_index
            };
            alias_indices.push(idx);
        }
        let unique_bits = FixedBitSet::with_capacity(unique_aliases.len());

        Ok(Self {
            pat1: enc_pat1,
            pat2: enc_pat2,
            pat: enc_pat,
            n_pat1: pat1.len(),
            n_pat2: pat2.len(),
            k,
            inexact,
            searcher_1,
            searcher_2,
            searcher,
            all_bits,
            unique_bits,
            unique_aliases,
            alias_indices,
        })
    }

    fn reset_bits(&mut self) {
        self.all_bits.clear();
        self.unique_bits.clear();
    }

    fn match_primary(&mut self, sequence: &[u8]) {
        if let Some(ref epat) = self.pat1 {
            search(
                &mut self.searcher_1,
                epat,
                sequence,
                &mut self.all_bits,
                self.k,
                self.inexact,
                0,
            );
        }
    }

    fn match_secondary(&mut self, sequence: &[u8]) {
        if let Some(ref epat) = self.pat2 {
            search(
                &mut self.searcher_2,
                epat,
                sequence,
                &mut self.all_bits,
                self.k,
                self.inexact,
                self.n_pat1,
            );
        }
    }

    fn match_either(&mut self, primary: &[u8], secondary: &[u8]) {
        if let Some(ref epat) = self.pat {
            let offset = self.n_pat1 + self.n_pat2;
            search(
                &mut self.searcher,
                epat,
                primary,
                &mut self.all_bits,
                self.k,
                self.inexact,
                offset,
            );
            search(
                &mut self.searcher,
                epat,
                secondary,
                &mut self.all_bits,
                self.k,
                self.inexact,
                offset,
            );
        }
    }
}

impl SequenceSplit for FuzzySplitter {
    fn split_idx(&mut self, primary: &[u8], secondary: &[u8]) -> Option<usize> {
        self.reset_bits();
        self.match_primary(primary);
        self.match_secondary(secondary);
        self.match_either(primary, secondary);

        self.all_bits.ones().for_each(|idx| {
            if let Some(u_idx) = self.alias_indices.get(idx) {
                self.unique_bits.set(*u_idx, true);
            }
        });

        get_single_hit(&self.unique_bits)
    }

    fn aliases(&self) -> &[String] {
        &self.unique_aliases
    }
}

fn search(
    searcher: &mut Searcher<Profile>,
    patterns: &EncodedPatterns<Profile>,
    sequence: &[u8],
    bitset: &mut FixedBitSet,
    k: usize,
    inexact: bool,
    offset: usize,
) {
    if sequence.is_empty() {
        return;
    }
    searcher
        .search_encoded_patterns(patterns, sequence, k)
        .iter()
        .for_each(|m: &Match| {
            if inexact && m.cost == 0 {
                return;
            }
            bitset.set(offset + m.pattern_idx, true);
        });
}

fn get_single_hit(bitset: &FixedBitSet) -> Option<usize> {
    let mut num_hits = 0;
    let match_id = bitset
        .ones()
        .inspect(|_idx| {
            num_hits += 1;
        })
        .last();

    if num_hits == 1 {
        match_id
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::FuzzySplitter;
    use crate::commands::grep::Pattern;
    use crate::commands::{grep::PatternCollection, split::splitter::SequenceSplit};

    fn pc(patterns: &[&[u8]], name: &str) -> PatternCollection {
        PatternCollection(
            patterns
                .iter()
                .map(|p| Pattern {
                    name: Some(name.to_string()),
                    sequence: p.to_vec(),
                })
                .collect(),
        )
    }

    // Mirrors https://github.com/RagnarGrootKoerkamp/sassy/issues/66: the Iupac
    // profile treats `N` as a wildcard, so without an N-fraction filter a needle
    // matches a haystack made entirely of `N`s.
    #[test]
    fn test_fuzzy_splitter_default_max_n_frac_rejects_all_n_match() {
        let pat1 = pc(&[b"ACGTACGTACGT"], "alias");
        let empty = PatternCollection(vec![]);
        let mut splitter = FuzzySplitter::new(&pat1, &empty, &empty, 1, false, None).unwrap();

        let all_n = b"NNNNNNNNNNNNNNNNNN";
        assert_eq!(
            splitter.split_idx(all_n, b""),
            None,
            "default max_n_frac (k/pattern_len) should reject an all-N match"
        );
    }

    #[test]
    fn test_fuzzy_splitter_max_n_frac_override_allows_all_n_match() {
        let pat1 = pc(&[b"ACGTACGTACGT"], "alias");
        let empty = PatternCollection(vec![]);
        let mut splitter = FuzzySplitter::new(&pat1, &empty, &empty, 1, false, Some(1.0)).unwrap();

        let all_n = b"NNNNNNNNNNNNNNNNNN";
        assert_eq!(
            splitter.split_idx(all_n, b""),
            Some(0),
            "max_n_frac=1.0 should disable the N-fraction filter"
        );
    }

    // sassy's `Searcher::encode_patterns` panics (`assert!`) when a pattern set
    // contains mixed lengths; these tests confirm we catch that up front and
    // return an `Err` instead of letting the panic reach the caller.
    #[test]
    fn test_fuzzy_splitter_rejects_mismatched_pattern_lengths_primary() {
        let pat1 = pc(&[b"AAAA", b"AAAAA"], "alias");
        let empty = PatternCollection(vec![]);
        let result = FuzzySplitter::new(&pat1, &empty, &empty, 1, false, None);
        assert!(
            result.is_err(),
            "mismatched primary pattern lengths should error, not panic"
        );
    }

    #[test]
    fn test_fuzzy_splitter_rejects_mismatched_pattern_lengths_secondary() {
        let pat2 = pc(&[b"AAAA", b"AAAAA"], "alias");
        let empty = PatternCollection(vec![]);
        let result = FuzzySplitter::new(&empty, &pat2, &empty, 1, false, None);
        assert!(
            result.is_err(),
            "mismatched secondary pattern lengths should error, not panic"
        );
    }

    #[test]
    fn test_fuzzy_splitter_rejects_mismatched_pattern_lengths_either() {
        let pat = pc(&[b"AAAA", b"AAAAA"], "alias");
        let empty = PatternCollection(vec![]);
        let result = FuzzySplitter::new(&empty, &empty, &pat, 1, false, None);
        assert!(
            result.is_err(),
            "mismatched either-set pattern lengths should error, not panic"
        );
    }

    #[test]
    fn test_fuzzy_splitter_accepts_uniform_pattern_lengths() {
        let pat1 = pc(&[b"AAAA", b"TTTT", b"CCCC"], "alias");
        let empty = PatternCollection(vec![]);
        let result = FuzzySplitter::new(&pat1, &empty, &empty, 1, false, None);
        assert!(result.is_ok(), "uniform pattern lengths should not error");
    }
}
