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

#[cfg(test)]
mod matcher_unit_tests {
    use std::collections::HashSet;

    use super::{PatternMatcher, RegexMatcher};

    #[cfg(feature = "fuzzy")]
    use super::FuzzyMatcher;

    #[test]
    fn test_regex_matcher_primary() {
        let re1 = vec![regex::bytes::Regex::new("AAAA").unwrap()];
        let mut matcher = RegexMatcher::new(re1, vec![], vec![]);

        let sequence = b"GGGGAAAATTTT";
        let mut matches = HashSet::new();

        let result = matcher.match_primary(sequence, &mut matches, true);

        assert!(result, "Should match pattern in sequence");
        assert!(!matches.is_empty(), "Should have match locations");
    }

    #[test]
    fn test_regex_matcher_secondary() {
        let re2 = vec![regex::bytes::Regex::new("TTTT").unwrap()];
        let mut matcher = RegexMatcher::new(vec![], re2, vec![]);

        let sequence = b"GGGGAAAATTTT";
        let mut matches = HashSet::new();

        let result = matcher.match_secondary(sequence, &mut matches, true);

        assert!(result, "Should match pattern in extended sequence");
        assert!(!matches.is_empty(), "Should have match locations");
    }

    #[test]
    fn test_regex_matcher_either() {
        let re = vec![regex::bytes::Regex::new("CCCC").unwrap()];
        let mut matcher = RegexMatcher::new(vec![], vec![], re);

        let primary = b"GGGGAAAATTTT";
        let secondary = b"GGGGCCCCTTTT";
        let mut smatches = HashSet::new();
        let mut xmatches = HashSet::new();

        let result = matcher.match_either(primary, secondary, &mut smatches, &mut xmatches, true);

        assert!(result, "Should match pattern in either sequence");
        assert!(smatches.is_empty(), "Should not match in primary");
        assert!(!xmatches.is_empty(), "Should match in extended");
    }

    #[test]
    fn test_regex_matcher_and_logic() {
        let re1 = vec![
            regex::bytes::Regex::new("AAAA").unwrap(),
            regex::bytes::Regex::new("TTTT").unwrap(),
        ];
        let mut matcher = RegexMatcher::new(re1, vec![], vec![]);

        // Sequence with both patterns
        let seq_both = b"GGGGAAAATTTT";
        let mut matches1 = HashSet::new();
        assert!(matcher.match_primary(seq_both, &mut matches1, true));

        // Sequence with only one pattern
        let seq_one = b"GGGGAAAACCCC";
        let mut matches2 = HashSet::new();
        assert!(!matcher.match_primary(seq_one, &mut matches2, true));
    }

    #[test]
    fn test_regex_matcher_or_logic() {
        let re1 = vec![
            regex::bytes::Regex::new("AAAA").unwrap(),
            regex::bytes::Regex::new("TTTT").unwrap(),
        ];
        let mut matcher = RegexMatcher::new(re1, vec![], vec![]);

        // Sequence with only one pattern
        let seq = b"GGGGAAAACCCC";
        let mut matches = HashSet::new();
        assert!(matcher.match_primary(seq, &mut matches, false));
    }

    #[test]
    fn test_regex_matcher_no_match() {
        let re1 = vec![regex::bytes::Regex::new("AAAA").unwrap()];
        let mut matcher = RegexMatcher::new(re1, vec![], vec![]);

        let sequence = b"GGGGCCCCTTTT";
        let mut matches = HashSet::new();

        let result = matcher.match_primary(sequence, &mut matches, true);

        assert!(!result, "Should not match pattern");
        assert!(matches.is_empty(), "Should have no match locations");
    }

    #[test]
    fn test_regex_matcher_multiple_matches() {
        let re1 = vec![regex::bytes::Regex::new("AA").unwrap()];
        let mut matcher = RegexMatcher::new(re1, vec![], vec![]);

        let sequence = b"AAGGAAGGAA";
        let mut matches = HashSet::new();

        let result = matcher.match_primary(sequence, &mut matches, true);

        assert!(result, "Should match pattern");
        assert!(matches.len() >= 3, "Should find multiple matches");
    }

    #[test]
    fn test_regex_matcher_anchors() {
        // Start anchor
        let re_start = vec![regex::bytes::Regex::new("^AAAA").unwrap()];
        let mut matcher_start = RegexMatcher::new(re_start, vec![], vec![]);

        let seq_match = b"AAAATTTT";
        let seq_no_match = b"GGGGAAAA";
        let mut matches1 = HashSet::new();
        let mut matches2 = HashSet::new();

        assert!(matcher_start.match_primary(seq_match, &mut matches1, true));
        assert!(!matcher_start.match_primary(seq_no_match, &mut matches2, true));

        // End anchor
        let re_end = vec![regex::bytes::Regex::new("TTTT$").unwrap()];
        let mut matcher_end = RegexMatcher::new(re_end, vec![], vec![]);

        let mut matches3 = HashSet::new();
        let mut matches4 = HashSet::new();

        assert!(matcher_end.match_primary(b"AAAATTTT", &mut matches3, true));
        assert!(!matcher_end.match_primary(b"TTTTGGGG", &mut matches4, true));
    }

    #[test]
    fn test_regex_matcher_character_classes() {
        let re1 = vec![regex::bytes::Regex::new("A[TC]G").unwrap()];
        let mut matcher = RegexMatcher::new(re1, vec![], vec![]);

        let seq1 = b"GGGGATGTTTTT";
        let seq2 = b"GGGGACGTTTTT";
        let mut matches1 = HashSet::new();
        let mut matches2 = HashSet::new();

        assert!(matcher.match_primary(seq1, &mut matches1, true));
        assert!(matcher.match_primary(seq2, &mut matches2, true));
    }

    #[test]
    fn test_regex_matcher_repetition() {
        let re1 = vec![regex::bytes::Regex::new("A{3,5}").unwrap()];
        let mut matcher = RegexMatcher::new(re1, vec![], vec![]);

        let seq_match = b"GGGGAAAATTTT";
        let seq_no_match = b"GGGGAATTTT";
        let mut matches1 = HashSet::new();
        let mut matches2 = HashSet::new();

        assert!(matcher.match_primary(seq_match, &mut matches1, true));
        assert!(!matcher.match_primary(seq_no_match, &mut matches2, true));
    }

    #[test]
    fn test_regex_matcher_alternation() {
        let re1 = vec![regex::bytes::Regex::new("(AA|TT){2}").unwrap()];
        let mut matcher = RegexMatcher::new(re1, vec![], vec![]);

        let seq1 = b"GGGGAAAATTTT";
        let seq2 = b"GGGGTTTTCCCC";
        let seq3 = b"GGGGAATTCCCC";
        let mut matches1 = HashSet::new();
        let mut matches2 = HashSet::new();
        let mut matches3 = HashSet::new();

        assert!(matcher.match_primary(seq1, &mut matches1, true));
        assert!(matcher.match_primary(seq2, &mut matches2, true));
        assert!(matcher.match_primary(seq3, &mut matches3, true));
    }

    #[test]
    fn test_regex_matcher_empty_patterns() {
        let mut matcher = RegexMatcher::new(vec![], vec![], vec![]);

        let sequence = b"GGGGAAAATTTT";
        let mut matches = HashSet::new();

        // Empty patterns should return true (match everything)
        assert!(matcher.match_primary(sequence, &mut matches, true));
        assert!(matcher.match_secondary(sequence, &mut matches, true));
    }

    #[test]
    fn test_regex_matcher_empty_sequence() {
        let re2 = vec![regex::bytes::Regex::new("AAAA").unwrap()];
        let mut matcher = RegexMatcher::new(vec![], re2, vec![]);

        let empty_seq = b"";
        let mut matches = HashSet::new();

        // Empty secondary sequence should return true (no requirement)
        assert!(matcher.match_secondary(empty_seq, &mut matches, true));
    }

    #[cfg(feature = "fuzzy")]
    #[test]
    fn test_fuzzy_matcher_basic() {
        let pat1 = vec![b"AAAAAAAA".to_vec()];
        let mut matcher = FuzzyMatcher::new(pat1, vec![], vec![], 1, false);

        // Exact match
        let seq_exact = b"GGGGAAAAAAAATTTT";
        let mut matches1 = HashSet::new();
        assert!(matcher.match_primary(seq_exact, &mut matches1, true));

        // One mismatch (within edit distance)
        let seq_mismatch = b"GGGGAAAAACAATTTT";
        let mut matches2 = HashSet::new();
        assert!(matcher.match_primary(seq_mismatch, &mut matches2, true));

        // Too many mismatches
        let seq_far = b"GGGGAAAACCAATTTT";
        let mut matches3 = HashSet::new();
        let result = matcher.match_primary(seq_far, &mut matches3, true);
        assert!(!result, "Unexpected match, >1 edit distance");
    }

    #[cfg(feature = "fuzzy")]
    #[test]
    fn test_fuzzy_matcher_edit_distances() {
        let pat1 = vec![b"AAAAAAAA".to_vec()];

        // Test with k=0 (exact match only)
        let mut matcher_k0 = FuzzyMatcher::new(pat1.clone(), vec![], vec![], 0, false);
        let seq_exact = b"GGGGAAAAAAAATTTT";
        let seq_mismatch = b"GGGGAAAAACAATTTT";
        let mut matches1 = HashSet::new();
        let mut matches2 = HashSet::new();

        assert!(matcher_k0.match_primary(seq_exact, &mut matches1, true));
        assert!(!matcher_k0.match_primary(seq_mismatch, &mut matches2, true));

        // Test with k=2 (up to 2 edits)
        let mut matcher_k2 = FuzzyMatcher::new(pat1, vec![], vec![], 2, false);
        let mut matches3 = HashSet::new();

        assert!(matcher_k2.match_primary(seq_mismatch, &mut matches3, true));
    }

    #[cfg(feature = "fuzzy")]
    #[test]
    fn test_fuzzy_matcher_inexact_only() {
        let pat1 = vec![b"AAAAAAAA".to_vec()];
        let mut matcher = FuzzyMatcher::new(pat1, vec![], vec![], 2, true);

        // Exact match should not be reported with inexact_only
        let seq_exact = b"GGGGAAAAAAAATTTT";
        let mut matches1 = HashSet::new();
        assert!(!matcher.match_primary(seq_exact, &mut matches1, true));

        // Inexact match should be reported
        let seq_inexact = b"GGGGAAAAACAATTTT";
        let mut matches2 = HashSet::new();
        assert!(matcher.match_primary(seq_inexact, &mut matches2, true));
    }

    #[cfg(feature = "fuzzy")]
    #[test]
    fn test_fuzzy_matcher_secondary() {
        let pat2 = vec![b"TTTTTTTT".to_vec()];
        let mut matcher = FuzzyMatcher::new(vec![], pat2, vec![], 1, false);

        let sequence = b"GGGGTTTTTTTTCCCC";
        let mut matches = HashSet::new();

        assert!(matcher.match_secondary(sequence, &mut matches, true));
    }

    #[cfg(feature = "fuzzy")]
    #[test]
    fn test_fuzzy_matcher_either() {
        let pat = vec![b"CCCCCCCC".to_vec()];
        let mut matcher = FuzzyMatcher::new(vec![], vec![], pat, 1, false);

        let primary = b"GGGGAAAATTTT";
        let secondary = b"GGGGCCCCCCCCTTTT";
        let mut smatches = HashSet::new();
        let mut xmatches = HashSet::new();

        let result = matcher.match_either(primary, secondary, &mut smatches, &mut xmatches, true);

        assert!(result, "Should match pattern in either sequence");
        assert!(!xmatches.is_empty(), "Should match in extended");
    }

    #[cfg(feature = "fuzzy")]
    #[test]
    fn test_fuzzy_matcher_and_logic() {
        let pat1 = vec![b"AAAAAAAA".to_vec(), b"TTTTTTTT".to_vec()];
        let mut matcher = FuzzyMatcher::new(pat1, vec![], vec![], 1, false);

        // Sequence with both patterns
        let seq_both = b"AAAAAAAATTTTTTTT";
        let mut matches1 = HashSet::new();
        assert!(matcher.match_primary(seq_both, &mut matches1, true));

        // Sequence with only one pattern
        let seq_one = b"AAAAAAAACCCCCCCC";
        let mut matches2 = HashSet::new();
        assert!(!matcher.match_primary(seq_one, &mut matches2, true));
    }

    #[cfg(feature = "fuzzy")]
    #[test]
    fn test_fuzzy_matcher_or_logic() {
        let pat1 = vec![b"AAAAAAAA".to_vec(), b"TTTTTTTT".to_vec()];
        let mut matcher = FuzzyMatcher::new(pat1, vec![], vec![], 1, false);

        // Sequence with only one pattern
        let seq = b"AAAAAAAACCCCCCCC";
        let mut matches = HashSet::new();
        assert!(matcher.match_primary(seq, &mut matches, false));
    }

    #[cfg(feature = "fuzzy")]
    #[test]
    fn test_fuzzy_matcher_empty_patterns() {
        let mut matcher = FuzzyMatcher::new(vec![], vec![], vec![], 1, false);

        let sequence = b"GGGGAAAATTTT";
        let mut matches = HashSet::new();

        // Empty patterns should return true
        assert!(matcher.match_primary(sequence, &mut matches, true));
        assert!(matcher.match_secondary(sequence, &mut matches, true));
    }

    #[test]
    fn test_match_location_tracking() {
        let re1 = vec![regex::bytes::Regex::new("AA").unwrap()];
        let mut matcher = RegexMatcher::new(re1, vec![], vec![]);

        let sequence = b"GGGAAATTTAAACCC";
        let mut matches = HashSet::new();

        matcher.match_primary(sequence, &mut matches, true);

        // Verify we have match locations
        assert!(!matches.is_empty());

        // Verify match locations are valid intervals
        for (start, end) in &matches {
            assert!(start < end, "Start should be before end");
            assert!(
                *end <= sequence.len(),
                "End should not exceed sequence length"
            );
        }
    }

    #[test]
    fn test_complex_pattern_combinations() {
        // Test combining primary, secondary, and either patterns
        let re1 = vec![regex::bytes::Regex::new("AAAA").unwrap()];
        let re2 = vec![regex::bytes::Regex::new("TTTT").unwrap()];
        let re = vec![regex::bytes::Regex::new("CCCC").unwrap()];

        let mut matcher = RegexMatcher::new(re1, re2, re);

        let primary = b"AAAAGGGGCCCCGGGG";
        let secondary = b"GGGGTTTTGGGGCCCC";

        let mut sm = HashSet::new();
        let mut xm = HashSet::new();

        // All patterns should match
        assert!(matcher.match_primary(primary, &mut sm, true));
        assert!(matcher.match_secondary(secondary, &mut xm, true));

        let mut sm2 = HashSet::new();
        let mut xm2 = HashSet::new();
        assert!(matcher.match_either(primary, secondary, &mut sm2, &mut xm2, true));
    }
}
