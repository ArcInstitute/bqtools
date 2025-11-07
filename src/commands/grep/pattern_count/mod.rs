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

pub trait PatternCount: Clone + Send + Sync {
    fn count_patterns(&mut self, primary: &[u8], secondary: &[u8], pattern_count: &mut [usize]);

    fn num_patterns(&self) -> usize;

    fn pattern_strings(&self) -> Vec<String>;
}

#[cfg(test)]
mod pattern_count_tests {
    use super::{PatternCount, RegexPatternCounter};

    #[cfg(feature = "fuzzy")]
    use super::FuzzyPatternCounter;

    #[test]
    fn test_regex_pattern_counter_single_pattern() {
        let re1 = vec![regex::bytes::Regex::new("AAAA").unwrap()];
        let mut counter = RegexPatternCounter::new(re1, vec![], vec![], false);

        assert_eq!(counter.num_patterns(), 1);

        let primary = b"GGGGAAAATTTT";
        let secondary = b"GGGGCCCCTTTT";
        let mut counts = vec![0; counter.num_patterns()];

        counter.count_patterns(primary, secondary, &mut counts);

        assert_eq!(counts[0], 1, "Pattern should be found in primary");
    }

    #[test]
    fn test_regex_pattern_counter_multiple_patterns() {
        let re1 = vec![
            regex::bytes::Regex::new("AAAA").unwrap(),
            regex::bytes::Regex::new("TTTT").unwrap(),
            regex::bytes::Regex::new("CCCC").unwrap(),
        ];
        let mut counter = RegexPatternCounter::new(re1, vec![], vec![], false);

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
        let re2 = vec![regex::bytes::Regex::new("TTTT").unwrap()];
        let mut counter = RegexPatternCounter::new(vec![], re2, vec![], false);

        let primary = b"GGGGAAAACCCC";
        let secondary = b"GGGGTTTTCCCC";
        let mut counts = vec![0; counter.num_patterns()];

        counter.count_patterns(primary, secondary, &mut counts);

        assert_eq!(counts[0], 1, "Pattern should be found in secondary");
    }

    #[test]
    fn test_regex_pattern_counter_either() {
        let re = vec![regex::bytes::Regex::new("CCCC").unwrap()];
        let mut counter = RegexPatternCounter::new(vec![], vec![], re, false);

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
        let re1 = vec![regex::bytes::Regex::new("AAAA").unwrap()];
        let mut counter = RegexPatternCounter::new(re1, vec![], vec![], false);

        let primary = b"GGGGCCCCTTTT";
        let secondary = b"GGGGCCCCTTTT";
        let mut counts = vec![0; counter.num_patterns()];

        counter.count_patterns(primary, secondary, &mut counts);

        assert_eq!(counts[0], 0, "Pattern should not be found");
    }

    #[test]
    fn test_regex_pattern_counter_invert() {
        let re1 = vec![regex::bytes::Regex::new("AAAA").unwrap()];
        let mut counter = RegexPatternCounter::new(re1, vec![], vec![], true);

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
        let re1 = vec![regex::bytes::Regex::new("AAAA").unwrap()];
        let re2 = vec![regex::bytes::Regex::new("TTTT").unwrap()];
        let re = vec![regex::bytes::Regex::new("CCCC").unwrap()];

        let mut counter = RegexPatternCounter::new(re1, re2, re, false);

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
        let re1 = vec![
            regex::bytes::Regex::new("AAAA").unwrap(),
            regex::bytes::Regex::new("TTTT").unwrap(),
        ];
        let counter = RegexPatternCounter::new(re1, vec![], vec![], false);

        let patterns = counter.pattern_strings();
        assert_eq!(patterns.len(), 2);
        assert_eq!(patterns[0], "AAAA");
        assert_eq!(patterns[1], "TTTT");
    }

    #[test]
    fn test_regex_pattern_counter_multiple_records() {
        let re1 = vec![regex::bytes::Regex::new("AAAA").unwrap()];
        let mut counter = RegexPatternCounter::new(re1, vec![], vec![], false);

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
        let re2 = vec![regex::bytes::Regex::new("AAAA").unwrap()];
        let mut counter = RegexPatternCounter::new(vec![], re2, vec![], false);

        let primary = b"GGGGAAAATTTT";
        let secondary = b"";
        let mut counts = vec![0; counter.num_patterns()];

        counter.count_patterns(primary, secondary, &mut counts);

        assert_eq!(counts[0], 0, "Should not count in empty secondary");
    }

    #[cfg(feature = "fuzzy")]
    #[test]
    fn test_fuzzy_pattern_counter_single_pattern() {
        let pat1 = vec![b"AAAAAAAA".to_vec()];
        let mut counter = FuzzyPatternCounter::new(pat1, vec![], vec![], 1, false, false);

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
        let pat1 = vec![b"AAAAAAAA".to_vec()];
        let mut counter = FuzzyPatternCounter::new(pat1, vec![], vec![], 2, false, false);

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
        let pat1 = vec![b"AAAAAAAA".to_vec()];
        let mut counter = FuzzyPatternCounter::new(pat1, vec![], vec![], 2, true, false);

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
        let pat1 = vec![b"AAAAAAAA".to_vec()];
        let mut counter = FuzzyPatternCounter::new(pat1, vec![], vec![], 1, false, true);

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
        let pat1 = vec![
            b"AAAAAAAA".to_vec(),
            b"TTTTTTTT".to_vec(),
            b"CCCCCCCC".to_vec(),
        ];
        let mut counter = FuzzyPatternCounter::new(pat1, vec![], vec![], 1, false, false);

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
        let pat2 = vec![b"TTTTTTTT".to_vec()];
        let mut counter = FuzzyPatternCounter::new(vec![], pat2, vec![], 1, false, false);

        let primary = b"GGGGAAAACCCC";
        let secondary = b"GGGGTTTTTTTTCCCC";
        let mut counts = vec![0; counter.num_patterns()];

        counter.count_patterns(primary, secondary, &mut counts);

        assert_eq!(counts[0], 1, "Pattern should be found in secondary");
    }

    #[cfg(feature = "fuzzy")]
    #[test]
    fn test_fuzzy_pattern_counter_either() {
        let pat = vec![b"CCCCCCCC".to_vec()];
        let mut counter = FuzzyPatternCounter::new(vec![], vec![], pat, 1, false, false);

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
        let pat1 = vec![b"AAAAAAAA".to_vec(), b"TTTTTTTT".to_vec()];
        let counter = FuzzyPatternCounter::new(pat1, vec![], vec![], 1, false, false);

        let patterns = counter.pattern_strings();
        assert_eq!(patterns.len(), 2);
        assert_eq!(patterns[0], "AAAAAAAA");
        assert_eq!(patterns[1], "TTTTTTTT");
    }

    #[cfg(feature = "fuzzy")]
    #[test]
    fn test_fuzzy_pattern_counter_edit_distance_zero() {
        let pat1 = vec![b"AAAAAAAA".to_vec()];
        let mut counter = FuzzyPatternCounter::new(pat1, vec![], vec![], 0, false, false);

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
}
