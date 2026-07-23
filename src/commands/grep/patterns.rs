use anyhow::Result;

/// Returns true if the pattern is a fixed DNA string (only ACGT).
pub(crate) fn is_fixed(pattern: &[u8]) -> bool {
    !pattern.is_empty()
        && pattern
            .iter()
            .all(|b| matches!(b, b'A' | b'C' | b'G' | b'T'))
}

/// A pattern with an optional name (from FASTA headers).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pattern {
    pub name: Option<String>,
    pub sequence: Vec<u8>,
}
impl Pattern {
    /// Compile the sequence into a regex.
    pub fn to_regex(&self) -> Result<regex::bytes::Regex> {
        let seq_str = std::str::from_utf8(&self.sequence)?;
        Ok(regex::bytes::Regex::new(seq_str)?)
    }

    /// Reverse complement the pattern's sequence in place.
    ///
    /// Errors if the sequence is not a fixed literal ACGT string, since
    /// reverse-complementing a regex (or an IUPAC-ambiguous pattern) is undefined.
    pub fn reverse_complement(&mut self) -> Result<()> {
        if !is_fixed(&self.sequence) {
            anyhow::bail!(
                "Cannot reverse complement pattern '{}': --rc only supports fixed ACGT patterns, not regex",
                String::from_utf8_lossy(&self.sequence)
            );
        }
        self.sequence = self
            .sequence
            .iter()
            .rev()
            .map(|b| match b {
                b'A' => b'T',
                b'T' => b'A',
                b'C' => b'G',
                b'G' => b'C',
                _ => unreachable!("is_fixed guarantees only ACGT bytes"),
            })
            .collect();
        Ok(())
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
        self.0.iter().map(Pattern::to_regex).collect()
    }

    /// Reverse complement every pattern in the collection, in place.
    ///
    /// Errors if any pattern is not a fixed literal ACGT string.
    pub fn reverse_complement(&mut self) -> Result<()> {
        for pattern in &mut self.0 {
            pattern.reverse_complement()?;
        }
        Ok(())
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

#[cfg(test)]
mod reverse_complement_tests {
    use super::Pattern;

    fn pattern(seq: &[u8]) -> Pattern {
        Pattern {
            name: None,
            sequence: seq.to_vec(),
        }
    }

    #[test]
    fn test_reverse_complement_basic() {
        let mut p = pattern(b"ACGT");
        p.reverse_complement().unwrap();
        assert_eq!(p.sequence, b"ACGT");

        let mut p = pattern(b"AACCGGTT");
        p.reverse_complement().unwrap();
        assert_eq!(p.sequence, b"AACCGGTT");

        let mut p = pattern(b"AAAA");
        p.reverse_complement().unwrap();
        assert_eq!(p.sequence, b"TTTT");

        let mut p = pattern(b"ACGTACGT");
        p.reverse_complement().unwrap();
        assert_eq!(p.sequence, b"ACGTACGT");

        let mut p = pattern(b"AGGT");
        p.reverse_complement().unwrap();
        assert_eq!(p.sequence, b"ACCT");
    }

    #[test]
    fn test_reverse_complement_preserves_name() {
        let mut p = Pattern {
            name: Some("my_pattern".to_string()),
            sequence: b"AGGT".to_vec(),
        };
        p.reverse_complement().unwrap();
        assert_eq!(p.name, Some("my_pattern".to_string()));
        assert_eq!(p.sequence, b"ACCT");
    }

    #[test]
    fn test_reverse_complement_rejects_regex() {
        assert!(pattern(b"AC.GT").reverse_complement().is_err());
        assert!(pattern(b"AC[GT]").reverse_complement().is_err());
        assert!(pattern(b"A{3}").reverse_complement().is_err());
        assert!(pattern(b"^ACGT").reverse_complement().is_err());
    }

    #[test]
    fn test_reverse_complement_rejects_iupac_ambiguity() {
        assert!(pattern(b"ACGN").reverse_complement().is_err());
    }

    #[test]
    fn test_reverse_complement_rejects_empty() {
        assert!(pattern(b"").reverse_complement().is_err());
    }
}
