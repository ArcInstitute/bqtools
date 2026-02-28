mod regex_pc;
pub use regex_pc::RegexPatternCounter;

mod ac_pc;
pub use ac_pc::AhoCorasickPatternCounter;

#[cfg(feature = "fuzzy")]
mod fuzzy_pc;
#[cfg(feature = "fuzzy")]
pub use fuzzy_pc::FuzzyPatternCounter;

mod processor;
pub use processor::PatternCountProcessor;

use super::PatternCollection;

pub trait PatternCount: Clone + Send + Sync {
    /// Counts the number of patterns in the given primary and secondary strings.
    ///
    /// Increments those specific pattern counts.
    ///
    /// Pattern counts are assumed to equal to the number of patterns found in (primary-only, secondary-only, either) expressions.
    /// The counts are indexed in that order in a single array.
    fn count_patterns(&mut self, primary: &[u8], secondary: &[u8], pattern_count: &mut [usize]);

    fn num_patterns(&self) -> usize;

    fn pattern_strings(&self) -> Vec<String>;

    /// Returns pattern names (FASTA headers if present, otherwise the pattern strings).
    fn pattern_names(&self) -> Vec<String>;
}

#[derive(Clone)]
pub enum PatternCounter {
    Regex(RegexPatternCounter),
    AhoCorasick(AhoCorasickPatternCounter),
    #[cfg(feature = "fuzzy")]
    Fuzzy(FuzzyPatternCounter),
}
impl PatternCount for PatternCounter {
    fn count_patterns(&mut self, primary: &[u8], secondary: &[u8], pattern_count: &mut [usize]) {
        match self {
            PatternCounter::Regex(counter) => {
                counter.count_patterns(primary, secondary, pattern_count);
            }
            PatternCounter::AhoCorasick(counter) => {
                counter.count_patterns(primary, secondary, pattern_count);
            }
            #[cfg(feature = "fuzzy")]
            PatternCounter::Fuzzy(counter) => {
                counter.count_patterns(primary, secondary, pattern_count)
            }
        }
    }

    fn num_patterns(&self) -> usize {
        match self {
            PatternCounter::Regex(counter) => counter.num_patterns(),
            PatternCounter::AhoCorasick(counter) => counter.num_patterns(),
            #[cfg(feature = "fuzzy")]
            PatternCounter::Fuzzy(counter) => counter.num_patterns(),
        }
    }

    fn pattern_strings(&self) -> Vec<String> {
        match self {
            PatternCounter::Regex(counter) => counter.pattern_strings(),
            PatternCounter::AhoCorasick(counter) => counter.pattern_strings(),
            #[cfg(feature = "fuzzy")]
            PatternCounter::Fuzzy(counter) => counter.pattern_strings(),
        }
    }

    fn pattern_names(&self) -> Vec<String> {
        match self {
            PatternCounter::Regex(counter) => counter.pattern_names(),
            PatternCounter::AhoCorasick(counter) => counter.pattern_names(),
            #[cfg(feature = "fuzzy")]
            PatternCounter::Fuzzy(counter) => counter.pattern_names(),
        }
    }
}

#[cfg(test)]
mod pattern_count_tests {
    use super::{AhoCorasickPatternCounter, PatternCount, RegexPatternCounter};
    use crate::commands::grep::{Pattern, PatternCollection};

    #[cfg(feature = "fuzzy")]
    use super::FuzzyPatternCounter;

    fn pc(patterns: &[&[u8]]) -> PatternCollection {
        PatternCollection(
            patterns
                .iter()
                .map(|p| Pattern {
                    name: None,
                    sequence: p.to_vec(),
                })
                .collect(),
        )
    }

    #[test]
    fn test_regex_pattern_counter_single_pattern() {
        let mut counter =
            RegexPatternCounter::new(pc(&[b"AAAA"]), pc(&[]), pc(&[]), false).unwrap();

        assert_eq!(counter.num_patterns(), 1);

        let primary = b"GGGGAAAATTTT";
        let secondary = b"GGGGCCCCTTTT";
        let mut counts = vec![0; counter.num_patterns()];

        counter.count_patterns(primary, secondary, &mut counts);

        assert_eq!(counts[0], 1, "Pattern should be found in primary");
    }

    #[test]
    fn test_regex_pattern_counter_multiple_patterns() {
        let mut counter =
            RegexPatternCounter::new(pc(&[b"AAAA", b"TTTT", b"CCCC"]), pc(&[]), pc(&[]), false)
                .unwrap();

        assert_eq!(counter.num_patterns(), 3);

        let primary = b"AAAAGGGGTTTT";
        let secondary = b"GGGGCCCCGGGG";
        let mut counts = vec![0; counter.num_patterns()];

        counter.count_patterns(primary, secondary, &mut counts);

        assert_eq!(counts[0], 1, "First pattern found");
        assert_eq!(counts[1], 1, "Second pattern found");
        assert_eq!(counts[2], 0, "Third pattern not found in primary");
    }

    #[test]
    fn test_regex_pattern_counter_secondary() {
        let mut counter =
            RegexPatternCounter::new(pc(&[]), pc(&[b"TTTT"]), pc(&[]), false).unwrap();

        let primary = b"GGGGAAAACCCC";
        let secondary = b"GGGGTTTTCCCC";
        let mut counts = vec![0; counter.num_patterns()];

        counter.count_patterns(primary, secondary, &mut counts);

        assert_eq!(counts[0], 1, "Pattern should be found in secondary");
    }

    #[test]
    fn test_regex_pattern_counter_either() {
        let mut counter =
            RegexPatternCounter::new(pc(&[]), pc(&[]), pc(&[b"CCCC"]), false).unwrap();

        // Test match in primary
        let primary1 = b"GGGGCCCCTTTT";
        let secondary1 = b"GGGGAAAATTTT";
        let mut counts1 = vec![0; counter.num_patterns()];
        counter.count_patterns(primary1, secondary1, &mut counts1);
        assert_eq!(counts1[0], 1);

        // Test match in secondary
        let primary2 = b"GGGGAAAATTTT";
        let secondary2 = b"GGGGCCCCTTTT";
        let mut counts2 = vec![0; counter.num_patterns()];
        counter.count_patterns(primary2, secondary2, &mut counts2);
        assert_eq!(counts2[0], 1);

        // Test match in both (should still count as 1)
        let primary3 = b"GGGGCCCCTTTT";
        let secondary3 = b"GGGGCCCCTTTT";
        let mut counts3 = vec![0; counter.num_patterns()];
        counter.count_patterns(primary3, secondary3, &mut counts3);
        assert_eq!(counts3[0], 1);
    }

    #[test]
    fn test_regex_pattern_counter_no_match() {
        let mut counter =
            RegexPatternCounter::new(pc(&[b"AAAA"]), pc(&[]), pc(&[]), false).unwrap();

        let primary = b"GGGGCCCCTTTT";
        let secondary = b"GGGGCCCCTTTT";
        let mut counts = vec![0; counter.num_patterns()];

        counter.count_patterns(primary, secondary, &mut counts);

        assert_eq!(counts[0], 0, "Pattern should not be found");
    }

    #[test]
    fn test_regex_pattern_counter_invert() {
        let mut counter = RegexPatternCounter::new(pc(&[b"AAAA"]), pc(&[]), pc(&[]), true).unwrap();

        // Sequence without pattern (should count when inverted)
        let primary1 = b"GGGGCCCCTTTT";
        let secondary1 = b"GGGGCCCCTTTT";
        let mut counts1 = vec![0; counter.num_patterns()];
        counter.count_patterns(primary1, secondary1, &mut counts1);
        assert_eq!(counts1[0], 1, "Should count when pattern not found");

        // Sequence with pattern (should not count when inverted)
        let primary2 = b"GGGGAAAATTTT";
        let secondary2 = b"GGGGCCCCTTTT";
        let mut counts2 = vec![0; counter.num_patterns()];
        counter.count_patterns(primary2, secondary2, &mut counts2);
        assert_eq!(counts2[0], 0, "Should not count when pattern found");
    }

    #[test]
    fn test_regex_pattern_counter_combined_patterns() {
        let mut counter =
            RegexPatternCounter::new(pc(&[b"AAAA"]), pc(&[b"TTTT"]), pc(&[b"CCCC"]), false)
                .unwrap();

        assert_eq!(counter.num_patterns(), 3);

        let primary = b"AAAACCCCGGGG";
        let secondary = b"GGGGTTTTCCCC";
        let mut counts = vec![0; counter.num_patterns()];

        counter.count_patterns(primary, secondary, &mut counts);

        assert_eq!(counts[0], 1, "Primary pattern found");
        assert_eq!(counts[1], 1, "Secondary pattern found");
        assert_eq!(counts[2], 1, "Either pattern found");
    }

    #[test]
    fn test_regex_pattern_counter_pattern_strings() {
        let counter =
            RegexPatternCounter::new(pc(&[b"AAAA", b"TTTT"]), pc(&[]), pc(&[]), false).unwrap();

        let patterns = counter.pattern_strings();
        assert_eq!(patterns.len(), 2);
        assert_eq!(patterns[0], "AAAA");
        assert_eq!(patterns[1], "TTTT");
    }

    #[test]
    fn test_regex_pattern_counter_multiple_records() {
        let mut counter =
            RegexPatternCounter::new(pc(&[b"AAAA"]), pc(&[]), pc(&[]), false).unwrap();

        let mut counts = vec![0; counter.num_patterns()];

        // Process multiple records
        let records = vec![
            (b"GGGGAAAATTTT" as &[u8], b"" as &[u8]),
            (b"GGGGCCCCTTTT", b""),
            (b"AAAACCCCGGGG", b""),
            (b"GGGGCCCCGGGG", b""),
            (b"AAAAAAAAAAAA", b""),
        ];

        for (primary, secondary) in records {
            counter.count_patterns(primary, secondary, &mut counts);
        }

        assert_eq!(
            counts[0], 3,
            "Pattern should be found in 3 out of 5 records"
        );
    }

    #[test]
    fn test_regex_pattern_counter_empty_sequence() {
        let mut counter =
            RegexPatternCounter::new(pc(&[]), pc(&[b"AAAA"]), pc(&[]), false).unwrap();

        let primary = b"GGGGAAAATTTT";
        let secondary = b"";
        let mut counts = vec![0; counter.num_patterns()];

        counter.count_patterns(primary, secondary, &mut counts);

        assert_eq!(counts[0], 0, "Should not count in empty secondary");
    }

    #[cfg(feature = "fuzzy")]
    #[test]
    fn test_fuzzy_pattern_counter_single_pattern() {
        let mut counter =
            FuzzyPatternCounter::new(pc(&[b"AAAAAAAA"]), pc(&[]), pc(&[]), 1, false, false);

        assert_eq!(counter.num_patterns(), 1);

        let primary = b"GGGGAAAAAAAATTTT";
        let secondary = b"GGGGCCCCTTTT";
        let mut counts = vec![0; counter.num_patterns()];

        counter.count_patterns(primary, secondary, &mut counts);

        assert_eq!(counts[0], 1, "Pattern should be found in primary");
    }

    #[cfg(feature = "fuzzy")]
    #[test]
    fn test_fuzzy_pattern_counter_with_mismatches() {
        let mut counter =
            FuzzyPatternCounter::new(pc(&[b"AAAAAAAA"]), pc(&[]), pc(&[]), 2, false, false);

        // Exact match
        let primary1 = b"GGGGAAAAAAAATTTT";
        let secondary1 = b"";
        let mut counts1 = vec![0; counter.num_patterns()];
        counter.count_patterns(primary1, secondary1, &mut counts1);
        assert_eq!(counts1[0], 1);

        // One mismatch
        let primary2 = b"GGGGAAAAACAATTTT";
        let secondary2 = b"";
        let mut counts2 = vec![0; counter.num_patterns()];
        counter.count_patterns(primary2, secondary2, &mut counts2);
        assert_eq!(counts2[0], 1);

        // Two mismatches
        let primary3 = b"GGGGAAAACCAATTTT";
        let secondary3 = b"";
        let mut counts3 = vec![0; counter.num_patterns()];
        counter.count_patterns(primary3, secondary3, &mut counts3);
        assert_eq!(counts3[0], 1);
    }

    #[cfg(feature = "fuzzy")]
    #[test]
    fn test_fuzzy_pattern_counter_inexact_only() {
        let mut counter =
            FuzzyPatternCounter::new(pc(&[b"AAAAAAAA"]), pc(&[]), pc(&[]), 2, true, false);

        // Exact match (should not count with inexact_only)
        let primary1 = b"GGGGAAAAAAAATTTT";
        let secondary1 = b"";
        let mut counts1 = vec![0; counter.num_patterns()];
        counter.count_patterns(primary1, secondary1, &mut counts1);
        assert_eq!(
            counts1[0], 0,
            "Exact match should not count with inexact_only"
        );

        // Inexact match (should count)
        let primary2 = b"GGGGAAAAACAATTTT";
        let secondary2 = b"";
        let mut counts2 = vec![0; counter.num_patterns()];
        counter.count_patterns(primary2, secondary2, &mut counts2);
        assert_eq!(counts2[0], 1, "Inexact match should count");
    }

    #[cfg(feature = "fuzzy")]
    #[test]
    fn test_fuzzy_pattern_counter_invert() {
        let mut counter =
            FuzzyPatternCounter::new(pc(&[b"AAAAAAAA"]), pc(&[]), pc(&[]), 1, false, true);

        // Sequence without pattern (should count when inverted)
        let primary1 = b"GGGGCCCCTTTT";
        let secondary1 = b"";
        let mut counts1 = vec![0; counter.num_patterns()];
        counter.count_patterns(primary1, secondary1, &mut counts1);
        assert_eq!(counts1[0], 1, "Should count when pattern not found");

        // Sequence with pattern (should not count when inverted)
        let primary2 = b"GGGGAAAAAAAATTTT";
        let secondary2 = b"";
        let mut counts2 = vec![0; counter.num_patterns()];
        counter.count_patterns(primary2, secondary2, &mut counts2);
        assert_eq!(counts2[0], 0, "Should not count when pattern found");
    }

    #[cfg(feature = "fuzzy")]
    #[test]
    fn test_fuzzy_pattern_counter_multiple_patterns() {
        let mut counter = FuzzyPatternCounter::new(
            pc(&[b"AAAAAAAA", b"TTTTTTTT", b"CCCCCCCC"]),
            pc(&[]),
            pc(&[]),
            1,
            false,
            false,
        );

        assert_eq!(counter.num_patterns(), 3);

        let primary = b"AAAAAAAATTTTTTTT";
        let secondary = b"";
        let mut counts = vec![0; counter.num_patterns()];

        counter.count_patterns(primary, secondary, &mut counts);

        assert_eq!(counts[0], 1, "First pattern found");
        assert_eq!(counts[1], 1, "Second pattern found");
        assert_eq!(counts[2], 0, "Third pattern not found");
    }

    #[cfg(feature = "fuzzy")]
    #[test]
    fn test_fuzzy_pattern_counter_secondary() {
        let mut counter =
            FuzzyPatternCounter::new(pc(&[]), pc(&[b"TTTTTTTT"]), pc(&[]), 1, false, false);

        let primary = b"GGGGAAAACCCC";
        let secondary = b"GGGGTTTTTTTTCCCC";
        let mut counts = vec![0; counter.num_patterns()];

        counter.count_patterns(primary, secondary, &mut counts);

        assert_eq!(counts[0], 1, "Pattern should be found in secondary");
    }

    #[cfg(feature = "fuzzy")]
    #[test]
    fn test_fuzzy_pattern_counter_either() {
        let mut counter =
            FuzzyPatternCounter::new(pc(&[]), pc(&[]), pc(&[b"CCCCCCCC"]), 1, false, false);

        // Test match in primary
        let primary1 = b"GGGGCCCCCCCCTTTT";
        let secondary1 = b"GGGGAAAATTTT";
        let mut counts1 = vec![0; counter.num_patterns()];
        counter.count_patterns(primary1, secondary1, &mut counts1);
        assert_eq!(counts1[0], 1);

        // Test match in secondary
        let primary2 = b"GGGGAAAATTTT";
        let secondary2 = b"GGGGCCCCCCCCTTTT";
        let mut counts2 = vec![0; counter.num_patterns()];
        counter.count_patterns(primary2, secondary2, &mut counts2);
        assert_eq!(counts2[0], 1);
    }

    #[cfg(feature = "fuzzy")]
    #[test]
    fn test_fuzzy_pattern_counter_pattern_strings() {
        let counter = FuzzyPatternCounter::new(
            pc(&[b"AAAAAAAA", b"TTTTTTTT"]),
            pc(&[]),
            pc(&[]),
            1,
            false,
            false,
        );

        let patterns = counter.pattern_strings();
        assert_eq!(patterns.len(), 2);
        assert_eq!(patterns[0], "AAAAAAAA");
        assert_eq!(patterns[1], "TTTTTTTT");
    }

    #[cfg(feature = "fuzzy")]
    #[test]
    fn test_fuzzy_pattern_counter_edit_distance_zero() {
        let mut counter =
            FuzzyPatternCounter::new(pc(&[b"AAAAAAAA"]), pc(&[]), pc(&[]), 0, false, false);

        // Exact match (should count)
        let primary1 = b"GGGGAAAAAAAATTTT";
        let secondary1 = b"";
        let mut counts1 = vec![0; counter.num_patterns()];
        counter.count_patterns(primary1, secondary1, &mut counts1);
        assert_eq!(counts1[0], 1);

        // One mismatch (should not count with k=0)
        let primary2 = b"GGGGAAAAACAATTTT";
        let secondary2 = b"";
        let mut counts2 = vec![0; counter.num_patterns()];
        counter.count_patterns(primary2, secondary2, &mut counts2);
        assert_eq!(counts2[0], 0);
    }

    #[test]
    fn test_aho_corasick_pattern_counter_single_pattern() {
        let mut counter =
            AhoCorasickPatternCounter::new(pc(&[b"AAAA"]), pc(&[]), pc(&[]), false, false).unwrap();

        assert_eq!(counter.num_patterns(), 1);

        let primary = b"GGGGAAAATTTT";
        let secondary = b"GGGGCCCCTTTT";
        let mut counts = vec![0; counter.num_patterns()];

        counter.count_patterns(primary, secondary, &mut counts);

        assert_eq!(counts[0], 1, "Pattern should be found in primary");
    }

    #[test]
    fn test_aho_corasick_pattern_counter_multiple_patterns() {
        let mut counter = AhoCorasickPatternCounter::new(
            pc(&[b"AAAA", b"TTTT", b"CCCC"]),
            pc(&[]),
            pc(&[]),
            false,
            false,
        )
        .unwrap();

        assert_eq!(counter.num_patterns(), 3);

        let primary = b"AAAAGGGGTTTT";
        let secondary = b"GGGGCCCCGGGG";
        let mut counts = vec![0; counter.num_patterns()];

        counter.count_patterns(primary, secondary, &mut counts);

        assert_eq!(counts[0], 1, "First pattern found");
        assert_eq!(counts[1], 1, "Second pattern found");
        assert_eq!(counts[2], 0, "Third pattern not found in primary");
    }

    #[test]
    fn test_aho_corasick_pattern_counter_secondary() {
        let mut counter =
            AhoCorasickPatternCounter::new(pc(&[]), pc(&[b"TTTT"]), pc(&[]), false, false).unwrap();

        let primary = b"GGGGAAAACCCC";
        let secondary = b"GGGGTTTTCCCC";
        let mut counts = vec![0; counter.num_patterns()];

        counter.count_patterns(primary, secondary, &mut counts);

        assert_eq!(counts[0], 1, "Pattern should be found in secondary");
    }

    #[test]
    fn test_aho_corasick_pattern_counter_either() {
        let mut counter =
            AhoCorasickPatternCounter::new(pc(&[]), pc(&[]), pc(&[b"CCCC"]), false, false).unwrap();

        // Test match in primary
        let primary1 = b"GGGGCCCCTTTT";
        let secondary1 = b"GGGGAAAATTTT";
        let mut counts1 = vec![0; counter.num_patterns()];
        counter.count_patterns(primary1, secondary1, &mut counts1);
        assert_eq!(counts1[0], 1);

        // Test match in secondary
        let primary2 = b"GGGGAAAATTTT";
        let secondary2 = b"GGGGCCCCTTTT";
        let mut counts2 = vec![0; counter.num_patterns()];
        counter.count_patterns(primary2, secondary2, &mut counts2);
        assert_eq!(counts2[0], 1);

        // Test match in both (should still count as 1)
        let primary3 = b"GGGGCCCCTTTT";
        let secondary3 = b"GGGGCCCCTTTT";
        let mut counts3 = vec![0; counter.num_patterns()];
        counter.count_patterns(primary3, secondary3, &mut counts3);
        assert_eq!(counts3[0], 1);
    }

    #[test]
    fn test_aho_corasick_pattern_counter_no_match() {
        let mut counter =
            AhoCorasickPatternCounter::new(pc(&[b"AAAA"]), pc(&[]), pc(&[]), false, false).unwrap();

        let primary = b"GGGGCCCCTTTT";
        let secondary = b"GGGGCCCCTTTT";
        let mut counts = vec![0; counter.num_patterns()];

        counter.count_patterns(primary, secondary, &mut counts);

        assert_eq!(counts[0], 0, "Pattern should not be found");
    }

    #[test]
    fn test_aho_corasick_pattern_counter_invert() {
        let mut counter =
            AhoCorasickPatternCounter::new(pc(&[b"AAAA"]), pc(&[]), pc(&[]), false, true).unwrap();

        // Sequence without pattern (should count when inverted)
        let primary1 = b"GGGGCCCCTTTT";
        let secondary1 = b"GGGGCCCCTTTT";
        let mut counts1 = vec![0; counter.num_patterns()];
        counter.count_patterns(primary1, secondary1, &mut counts1);
        assert_eq!(counts1[0], 1, "Should count when pattern not found");

        // Sequence with pattern (should not count when inverted)
        let primary2 = b"GGGGAAAATTTT";
        let secondary2 = b"GGGGCCCCTTTT";
        let mut counts2 = vec![0; counter.num_patterns()];
        counter.count_patterns(primary2, secondary2, &mut counts2);
        assert_eq!(counts2[0], 0, "Should not count when pattern found");
    }

    #[test]
    fn test_aho_corasick_pattern_counter_combined_patterns() {
        let mut counter = AhoCorasickPatternCounter::new(
            pc(&[b"AAAA"]),
            pc(&[b"TTTT"]),
            pc(&[b"CCCC"]),
            false,
            false,
        )
        .unwrap();

        assert_eq!(counter.num_patterns(), 3);

        let primary = b"AAAACCCCGGGG";
        let secondary = b"GGGGTTTTCCCC";
        let mut counts = vec![0; counter.num_patterns()];

        counter.count_patterns(primary, secondary, &mut counts);

        assert_eq!(counts[0], 1, "Primary pattern found");
        assert_eq!(counts[1], 1, "Secondary pattern found");
        assert_eq!(counts[2], 1, "Either pattern found");
    }

    #[test]
    fn test_aho_corasick_pattern_counter_pattern_strings() {
        let counter =
            AhoCorasickPatternCounter::new(pc(&[b"AAAA", b"TTTT"]), pc(&[]), pc(&[]), false, false)
                .unwrap();

        let patterns = counter.pattern_strings();
        assert_eq!(patterns.len(), 2);
        assert_eq!(patterns[0], "AAAA");
        assert_eq!(patterns[1], "TTTT");
    }

    #[test]
    fn test_aho_corasick_pattern_counter_multiple_records() {
        let mut counter =
            AhoCorasickPatternCounter::new(pc(&[b"AAAA"]), pc(&[]), pc(&[]), false, false).unwrap();

        let mut counts = vec![0; counter.num_patterns()];

        // Process multiple records
        let records = vec![
            (b"GGGGAAAATTTT" as &[u8], b"" as &[u8]),
            (b"GGGGCCCCTTTT", b""),
            (b"AAAACCCCGGGG", b""),
            (b"GGGGCCCCGGGG", b""),
            (b"AAAAAAAAAAAA", b""),
        ];

        for (primary, secondary) in records {
            counter.count_patterns(primary, secondary, &mut counts);
        }

        assert_eq!(
            counts[0], 3,
            "Pattern should be found in 3 out of 5 records"
        );
    }

    #[test]
    fn test_aho_corasick_pattern_counter_empty_sequence() {
        let mut counter =
            AhoCorasickPatternCounter::new(pc(&[]), pc(&[b"AAAA"]), pc(&[]), false, false).unwrap();

        let primary = b"GGGGAAAATTTT";
        let secondary = b"";
        let mut counts = vec![0; counter.num_patterns()];

        counter.count_patterns(primary, secondary, &mut counts);

        assert_eq!(counts[0], 0, "Should not count in empty secondary");
    }

    #[test]
    fn test_aho_corasick_pattern_counter_overlapping_patterns() {
        let mut counter =
            AhoCorasickPatternCounter::new(pc(&[b"AAA", b"AAAA"]), pc(&[]), pc(&[]), false, false)
                .unwrap();

        let primary = b"GGGGAAAAATTTT";
        let secondary = b"";
        let mut counts = vec![0; counter.num_patterns()];

        counter.count_patterns(primary, secondary, &mut counts);

        assert_eq!(counts[0], 1, "AAA pattern found");
        assert_eq!(counts[1], 1, "AAAA pattern found");
    }

    #[test]
    fn test_aho_corasick_pattern_counter_multiple_occurrences() {
        let mut counter =
            AhoCorasickPatternCounter::new(pc(&[b"AAA", b"AAAA"]), pc(&[]), pc(&[]), false, false)
                .unwrap();

        let primary = b"GGGGAAAAATTTT";
        let secondary = b"";
        let mut counts = vec![0; counter.num_patterns()];

        counter.count_patterns(primary, secondary, &mut counts);

        // Both patterns should be found (overlapping matches)
        assert_eq!(counts[0], 1, "AAA pattern found");
        assert_eq!(counts[1], 1, "AAAA pattern found");
    }

    #[test]
    fn test_aho_corasick_pattern_counter_case_sensitive() {
        let mut counter =
            AhoCorasickPatternCounter::new(pc(&[b"aaaa"]), pc(&[]), pc(&[]), false, false).unwrap();

        // Different case should not match
        let primary = b"GGGGAAAATTTT";
        let secondary = b"";
        let mut counts = vec![0; counter.num_patterns()];

        counter.count_patterns(primary, secondary, &mut counts);

        assert_eq!(counts[0], 0, "Case sensitive pattern should not match");

        // Same case should match
        let primary2 = b"ggggaaaatttt";
        let secondary2 = b"";
        let mut counts2 = vec![0; counter.num_patterns()];

        counter.count_patterns(primary2, secondary2, &mut counts2);

        assert_eq!(counts2[0], 1, "Same case should match");
    }
}
