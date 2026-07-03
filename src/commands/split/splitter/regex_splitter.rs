use anyhow::Result;
use fixedbitset::FixedBitSet;
use hashbrown::HashMap;

use crate::commands::{grep::PatternCollection, split::splitter::SequenceSplit};

type Expressions = Vec<regex::bytes::Regex>;

/// Splits records into output bins using regular-expression matching.
///
/// Patterns are matched against the primary sequence (`re1`), the secondary
/// sequence (`re2`), and either sequence (`re`). A record is assigned to a bin
/// only when its matches resolve to exactly one unique alias.
#[derive(Clone)]
pub struct RegexSplitter {
    re1: Expressions,
    re2: Expressions,
    re: Expressions,

    /// bitset over all patterns
    all_bits: FixedBitSet,

    /// bitset over all unique aliases
    unique_bits: FixedBitSet,

    /// unique aliases across all pattern sets
    unique_aliases: Vec<String>,

    /// points to which alias is present at global pattern index
    alias_indices: Vec<usize>,
}

impl RegexSplitter {
    pub fn new(
        pat1: &PatternCollection,
        pat2: &PatternCollection,
        pat: &PatternCollection,
    ) -> Result<Self> {
        let re1 = pat1.regexes()?;
        let re2 = pat2.regexes()?;
        let re = pat.regexes()?;

        let all_bits = FixedBitSet::with_capacity(pat1.len() + pat2.len() + pat.len());

        let mut alias_indices = Vec::new();
        let mut unique_aliases = Vec::new();
        let mut map = HashMap::new();
        for name in pat1
            .names()
            .into_iter()
            .chain(pat2.names().into_iter())
            .chain(pat.names().into_iter())
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
            re1,
            re2,
            re,
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
        match_patterns(&self.re1, &mut self.all_bits, sequence, None, 0);
    }

    fn match_secondary(&mut self, sequence: &[u8]) {
        match_patterns(
            &self.re2,
            &mut self.all_bits,
            sequence,
            None,
            self.re1.len(),
        );
    }

    fn match_either(&mut self, primary: &[u8], secondary: &[u8]) {
        match_patterns(
            &self.re,
            &mut self.all_bits,
            primary,
            Some(secondary),
            self.re1.len() + self.re2.len(),
        );
    }
}

impl SequenceSplit for RegexSplitter {
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

fn match_patterns(
    patterns: &Expressions,
    bitset: &mut FixedBitSet,
    seq_a: &[u8],
    seq_b: Option<&[u8]>,
    offset: usize,
) {
    if patterns.is_empty() {
        return;
    }

    let mut fill_bitset = |seq: &[u8]| {
        if !seq.is_empty() {
            patterns.iter().enumerate().for_each(|(idx, reg)| {
                if reg.find(seq).is_some() {
                    bitset.set(offset + idx, true);
                }
            });
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
