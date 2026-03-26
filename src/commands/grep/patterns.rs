use anyhow::Result;

/// A pattern with an optional name (from FASTA headers).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pattern {
    pub name: Option<String>,
    pub sequence: Vec<u8>,
}
impl Pattern {
    /// Compile the sequence into a regex.
    pub fn into_regex(&self) -> Result<regex::bytes::Regex> {
        let seq_str = std::str::from_utf8(&self.sequence)?;
        Ok(regex::bytes::Regex::new(seq_str)?)
    }
}

/// A collection of patterns with convenience methods for type conversions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PatternCollection(pub Vec<Pattern>);
impl PatternCollection {
    pub fn bytes(&self) -> Vec<Vec<u8>> {
        self.0.iter().map(|p| p.sequence.clone()).collect()
    }

    pub fn regexes(&self) -> Result<Vec<regex::bytes::Regex>> {
        self.0.iter().map(Pattern::into_regex).collect()
    }

    pub fn names(&self) -> Vec<String> {
        self.0
            .iter()
            .map(|p| {
                p.name.clone().unwrap_or_else(|| {
                    std::str::from_utf8(&p.sequence)
                        .expect("Non-UTF8 sequence in pattern")
                        .to_string()
                })
            })
            .collect()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn has_patterns(&self) -> bool {
        !self.is_empty()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Pattern> {
        self.0.iter()
    }

    /// Takes all patterns from `other` and moves them into this collection.
    pub fn ingest(&mut self, other: &mut Self) {
        self.0.extend(other.drain());
    }

    /// Drains all patterns from this collection, returning an iterator over them.
    pub fn drain(&mut self) -> impl Iterator<Item = Pattern> + '_ {
        self.0.drain(..)
    }

    /// Clears all patterns from this collection.
    pub fn clear(&mut self) {
        self.0.clear();
    }
}
impl IntoIterator for PatternCollection {
    type Item = Pattern;
    type IntoIter = std::vec::IntoIter<Pattern>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}
