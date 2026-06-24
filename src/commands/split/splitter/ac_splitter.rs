use std::collections::HashMap;

use aho_corasick::{AhoCorasick, AhoCorasickBuilder, AhoCorasickKind};
use anyhow::Result;
use fixedbitset::FixedBitSet;

use crate::commands::{grep::PatternCollection, split::splitter::SequenceSplit};

/// Splits records into output bins using Aho-Corasick fixed-string matching.
///
/// Patterns are matched against the primary sequence (`pat1`), the secondary
/// sequence (`pat2`), and either sequence (`pat`). A record is assigned to a bin
/// only when its matches resolve to exactly one unique alias.
#[derive(Clone)]
pub struct AhoCorasickSplitter {
    state1: AhoCorasick,
    state2: AhoCorasick,
    state: AhoCorasick,

    /// bitset over all patterns
    all_bits: FixedBitSet,

    /// bitset over all unique aliases
    unique_bits: FixedBitSet,

    /// unique aliases across all pattern sets
    unique_aliases: Vec<String>,

    /// points to which alias is present at global pattern index
    alias_indices: Vec<usize>,
}

impl AhoCorasickSplitter {
    pub fn new(
        pat1: &PatternCollection,
        pat2: &PatternCollection,
        pat: &PatternCollection,
        no_dfa: bool,
    ) -> Result<Self> {
        let state1 = corasick_builder(&pat1.bytes(), no_dfa)?;
        let state2 = corasick_builder(&pat2.bytes(), no_dfa)?;
        let state = corasick_builder(&pat.bytes(), no_dfa)?;

        let all_bits = FixedBitSet::with_capacity(pat1.len() + pat2.len() + pat.len());

        let mut alias_indices = Vec::new();
        let mut unique_aliases = Vec::new();
        let mut map = HashMap::new();
        for name in pat1
            .names()
            .iter()
            .chain(pat2.names().iter())
            .chain(pat.names().iter())
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
            state1,
            state2,
            state,
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
        match_patterns(&self.state1, &mut self.all_bits, sequence, None, 0);
    }

    fn match_secondary(&mut self, sequence: &[u8]) {
        match_patterns(
            &self.state2,
            &mut self.all_bits,
            sequence,
            None,
            self.state1.patterns_len(),
        );
    }

    fn match_either(&mut self, primary: &[u8], secondary: &[u8]) {
        match_patterns(
            &self.state,
            &mut self.all_bits,
            primary,
            Some(secondary),
            self.state1.patterns_len() + self.state2.patterns_len(),
        );
    }
}

impl SequenceSplit for AhoCorasickSplitter {
    fn split_idx(&mut self, primary: &[u8], secondary: &[u8]) -> Option<usize> {
        self.reset_bits();
        self.match_primary(primary);
        self.match_secondary(secondary);
        self.match_either(primary, secondary);

        self.all_bits.ones().for_each(|idx| {
            if let Some(u_idx) = self.alias_indices.get(idx) {
                self.unique_bits.set(*u_idx, true)
            }
        });

        get_single_hit(&self.unique_bits)
    }

    fn aliases(&self) -> &[String] {
        &self.unique_aliases
    }
}

fn match_patterns(
    patterns: &AhoCorasick,
    bitset: &mut FixedBitSet,
    seq_a: &[u8],
    seq_b: Option<&[u8]>,
    offset: usize,
) {
    if patterns.patterns_len() == 0 {
        return;
    }

    let mut fill_bitset = |seq: &[u8]| {
        if !seq.is_empty() {
            patterns
                .find_overlapping_iter(seq)
                .for_each(|m| bitset.set(offset + m.pattern().as_usize(), true));
        }
    };

    fill_bitset(seq_a);
    seq_b.map(fill_bitset);
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

fn corasick_builder(patterns: &[Vec<u8>], no_dfa: bool) -> Result<AhoCorasick> {
    Ok(AhoCorasickBuilder::new()
        .ascii_case_insensitive(false)
        .kind(if no_dfa {
            None
        } else {
            Some(AhoCorasickKind::DFA)
        })
        .build(patterns)?)
}
