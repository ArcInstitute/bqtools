use hashbrown::HashMap;

use anyhow::Result;
use fixedbitset::FixedBitSet;
use log::error;
use sassy::{profiles::Iupac, EncodedPatterns, Match, Searcher};

use crate::commands::{grep::PatternCollection, split::splitter::SequenceSplit};

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
    ) -> Result<Self> {
        // sassy requires uniform pattern lengths within a searcher
        validate_single_pattern_length(&pat1.bytes())?;
        validate_single_pattern_length(&pat2.bytes())?;
        validate_single_pattern_length(&pat.bytes())?;

        // initialize a searcher for each pattern collection
        let mut searcher_1 = Searcher::new_fwd();
        let mut searcher_2 = Searcher::new_fwd();
        let mut searcher = Searcher::new_fwd();

        // encode the patterns for each collection/searcher combination
        let enc_pat1 = (!pat1.is_empty()).then(|| searcher_1.encode_patterns(&pat1.bytes()));
        let enc_pat2 = (!pat2.is_empty()).then(|| searcher_2.encode_patterns(&pat2.bytes()));
        let enc_pat = (!pat.is_empty()).then(|| searcher.encode_patterns(&pat.bytes()));

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

fn validate_single_pattern_length(patterns: &[Vec<u8>]) -> Result<()> {
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
