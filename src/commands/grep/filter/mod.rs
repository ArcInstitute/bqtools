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

pub trait PatternMatch: Clone + Send + Sync {
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

    fn offset(&self) -> usize;
}

#[derive(Clone)]
pub enum PatternMatcher {
    Regex(RegexMatcher),
    #[cfg(feature = "fuzzy")]
    Fuzzy(FuzzyMatcher),
}
impl PatternMatch for PatternMatcher {
    fn match_primary(
        &mut self,
        sequence: &[u8],
        matches: &mut MatchRanges,
        and_logic: bool,
    ) -> bool {
        match self {
            PatternMatcher::Regex(ref mut matcher) => {
                matcher.match_primary(sequence, matches, and_logic)
            }
            #[cfg(feature = "fuzzy")]
            PatternMatcher::Fuzzy(ref mut matcher) => {
                matcher.match_primary(sequence, matches, and_logic)
            }
        }
    }

    fn match_secondary(
        &mut self,
        sequence: &[u8],
        matches: &mut MatchRanges,
        and_logic: bool,
    ) -> bool {
        match self {
            PatternMatcher::Regex(ref mut matcher) => {
                matcher.match_secondary(sequence, matches, and_logic)
            }
            #[cfg(feature = "fuzzy")]
            PatternMatcher::Fuzzy(ref mut matcher) => {
                matcher.match_secondary(sequence, matches, and_logic)
            }
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
        match self {
            PatternMatcher::Regex(ref mut matcher) => {
                matcher.match_either(primary, secondary, smatches, xmatches, and_logic)
            }
            #[cfg(feature = "fuzzy")]
            PatternMatcher::Fuzzy(ref mut matcher) => {
                matcher.match_either(primary, secondary, smatches, xmatches, and_logic)
            }
        }
    }

    fn offset(&self) -> usize {
        match self {
            PatternMatcher::Regex(ref matcher) => matcher.offset(),
            #[cfg(feature = "fuzzy")]
            PatternMatcher::Fuzzy(ref matcher) => matcher.offset(),
        }
    }
}

#[cfg(test)]
mod matcher_unit_tests {
    use std::collections::HashSet;

    use super::{PatternMatch, RegexMatcher};

    #[cfg(feature = "fuzzy")]
    use super::FuzzyMatcher;

    #[test]
    fn test_regex_matcher_primary() {
        let re1 = vec![regex::bytes::Regex::new("AAAA").unwrap()];
        let mut matcher = RegexMatcher::new(re1, vec![], vec![], 0);

        let sequence = b"GGGGAAAATTTT";
        let mut matches = HashSet::new();

        let result = matcher.match_primary(sequence, &mut matches, true);

        assert!(result, "Should match pattern in sequence");
        assert!(!matches.is_empty(), "Should have match locations");
    }

    #[test]
    fn test_regex_matcher_secondary() {
        let re2 = vec![regex::bytes::Regex::new("TTTT").unwrap()];
        let mut matcher = RegexMatcher::new(vec![], re2, vec![], 0);

        let sequence = b"GGGGAAAATTTT";
        let mut matches = HashSet::new();

        let result = matcher.match_secondary(sequence, &mut matches, true);

        assert!(result, "Should match pattern in extended sequence");
        assert!(!matches.is_empty(), "Should have match locations");
    }

    #[test]
    fn test_regex_matcher_either() {
        let re = vec![regex::bytes::Regex::new("CCCC").unwrap()];
        let mut matcher = RegexMatcher::new(vec![], vec![], re, 0);

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
        let mut matcher = RegexMatcher::new(re1, vec![], vec![], 0);

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
        let mut matcher = RegexMatcher::new(re1, vec![], vec![], 0);

        // Sequence with only one pattern
        let seq = b"GGGGAAAACCCC";
        let mut matches = HashSet::new();
        assert!(matcher.match_primary(seq, &mut matches, false));
    }

    #[test]
    fn test_regex_matcher_no_match() {
        let re1 = vec![regex::bytes::Regex::new("AAAA").unwrap()];
        let mut matcher = RegexMatcher::new(re1, vec![], vec![], 0);

        let sequence = b"GGGGCCCCTTTT";
        let mut matches = HashSet::new();

        let result = matcher.match_primary(sequence, &mut matches, true);

        assert!(!result, "Should not match pattern");
        assert!(matches.is_empty(), "Should have no match locations");
    }

    #[test]
    fn test_regex_matcher_multiple_matches() {
        let re1 = vec![regex::bytes::Regex::new("AA").unwrap()];
        let mut matcher = RegexMatcher::new(re1, vec![], vec![], 0);

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
        let mut matcher_start = RegexMatcher::new(re_start, vec![], vec![], 0);

        let seq_match = b"AAAATTTT";
        let seq_no_match = b"GGGGAAAA";
        let mut matches1 = HashSet::new();
        let mut matches2 = HashSet::new();

        assert!(matcher_start.match_primary(seq_match, &mut matches1, true));
        assert!(!matcher_start.match_primary(seq_no_match, &mut matches2, true));

        // End anchor
        let re_end = vec![regex::bytes::Regex::new("TTTT$").unwrap()];
        let mut matcher_end = RegexMatcher::new(re_end, vec![], vec![], 0);

        let mut matches3 = HashSet::new();
        let mut matches4 = HashSet::new();

        assert!(matcher_end.match_primary(b"AAAATTTT", &mut matches3, true));
        assert!(!matcher_end.match_primary(b"TTTTGGGG", &mut matches4, true));
    }

    #[test]
    fn test_regex_matcher_character_classes() {
        let re1 = vec![regex::bytes::Regex::new("A[TC]G").unwrap()];
        let mut matcher = RegexMatcher::new(re1, vec![], vec![], 0);

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
        let mut matcher = RegexMatcher::new(re1, vec![], vec![], 0);

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
        let mut matcher = RegexMatcher::new(re1, vec![], vec![], 0);

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
        let mut matcher = RegexMatcher::new(vec![], vec![], vec![], 0);

        let sequence = b"GGGGAAAATTTT";
        let mut matches = HashSet::new();

        // Empty patterns should return true (match everything)
        assert!(matcher.match_primary(sequence, &mut matches, true));
        assert!(matcher.match_secondary(sequence, &mut matches, true));
    }

    #[test]
    fn test_regex_matcher_empty_sequence() {
        let re2 = vec![regex::bytes::Regex::new("AAAA").unwrap()];
        let mut matcher = RegexMatcher::new(vec![], re2, vec![], 0);

        let empty_seq = b"";
        let mut matches = HashSet::new();

        // Empty secondary sequence should return true (no requirement)
        assert!(matcher.match_secondary(empty_seq, &mut matches, true));
    }

    #[cfg(feature = "fuzzy")]
    #[test]
    fn test_fuzzy_matcher_basic() {
        let pat1 = vec![b"AAAAAAAA".to_vec()];
        let mut matcher = FuzzyMatcher::new(pat1, vec![], vec![], 1, false, 0);

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
        let mut matcher_k0 = FuzzyMatcher::new(pat1.clone(), vec![], vec![], 0, false, 0);
        let seq_exact = b"GGGGAAAAAAAATTTT";
        let seq_mismatch = b"GGGGAAAAACAATTTT";
        let mut matches1 = HashSet::new();
        let mut matches2 = HashSet::new();

        assert!(matcher_k0.match_primary(seq_exact, &mut matches1, true));
        assert!(!matcher_k0.match_primary(seq_mismatch, &mut matches2, true));

        // Test with k=2 (up to 2 edits)
        let mut matcher_k2 = FuzzyMatcher::new(pat1, vec![], vec![], 2, false, 0);
        let mut matches3 = HashSet::new();

        assert!(matcher_k2.match_primary(seq_mismatch, &mut matches3, true));
    }

    #[cfg(feature = "fuzzy")]
    #[test]
    fn test_fuzzy_matcher_inexact_only() {
        let pat1 = vec![b"AAAAAAAA".to_vec()];
        let mut matcher = FuzzyMatcher::new(pat1, vec![], vec![], 2, true, 0);

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
        let mut matcher = FuzzyMatcher::new(vec![], pat2, vec![], 1, false, 0);

        let sequence = b"GGGGTTTTTTTTCCCC";
        let mut matches = HashSet::new();

        assert!(matcher.match_secondary(sequence, &mut matches, true));
    }

    #[cfg(feature = "fuzzy")]
    #[test]
    fn test_fuzzy_matcher_either() {
        let pat = vec![b"CCCCCCCC".to_vec()];
        let mut matcher = FuzzyMatcher::new(vec![], vec![], pat, 1, false, 0);

        let primary = b"GGGGAAAATTTT";
        let secondary = b"GGGGCCCCCCCCTTTT";
        let mut smatches = HashSet::new();
        let mut xmatches = HashSet::new();

        let result = matcher.match_either(primary, secondary, &mut smatches, &mut xmatches, true);

        assert!(result, "Should match pattern in either sequence");
        assert!(!xmatches.is_empty(), "Should match in extended");
        assert!(smatches.is_empty(), "Should match in extended");
    }

    #[cfg(feature = "fuzzy")]
    #[test]
    fn test_fuzzy_matcher_and_logic() {
        let pat1 = vec![b"AAAAAAAA".to_vec(), b"TTTTTTTT".to_vec()];
        let mut matcher = FuzzyMatcher::new(pat1, vec![], vec![], 1, false, 0);

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
        let mut matcher = FuzzyMatcher::new(pat1, vec![], vec![], 1, false, 0);

        // Sequence with only one pattern
        let seq = b"AAAAAAAACCCCCCCC";
        let mut matches = HashSet::new();
        assert!(matcher.match_primary(seq, &mut matches, false));
    }

    #[cfg(feature = "fuzzy")]
    #[test]
    fn test_fuzzy_matcher_empty_patterns() {
        let mut matcher = FuzzyMatcher::new(vec![], vec![], vec![], 1, false, 0);

        let sequence = b"GGGGAAAATTTT";
        let mut matches = HashSet::new();

        // Empty patterns should return true
        assert!(matcher.match_primary(sequence, &mut matches, true));
        assert!(matcher.match_secondary(sequence, &mut matches, true));
    }

    #[test]
    fn test_match_location_tracking() {
        let re1 = vec![regex::bytes::Regex::new("AA").unwrap()];
        let mut matcher = RegexMatcher::new(re1, vec![], vec![], 0);

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
    fn test_regex_matcher_offset_zero() {
        let re1 = vec![regex::bytes::Regex::new("AA").unwrap()];
        let mut matcher = RegexMatcher::new(re1, vec![], vec![], 0);

        let sequence = b"GGGGAATTTT";
        let mut matches = HashSet::new();

        matcher.match_primary(sequence, &mut matches, true);

        // With offset=0, positions should be original
        assert!(
            matches.contains(&(4, 6)),
            "Should find match at original position (4, 6)"
        );
        assert_eq!(matcher.offset(), 0);
    }

    #[test]
    fn test_regex_matcher_offset_nonzero() {
        let offset = 10;
        let re1 = vec![regex::bytes::Regex::new("AA").unwrap()];
        let mut matcher = RegexMatcher::new(re1, vec![], vec![], offset);

        let sequence = b"GGGGAATTTT";
        let mut matches = HashSet::new();

        matcher.match_primary(sequence, &mut matches, true);

        // With offset=10, positions should be adjusted
        assert!(
            matches.contains(&(14, 16)),
            "Should find match at offset position (14, 16)"
        );
        assert_eq!(matcher.offset(), offset);

        // Should NOT contain the original position
        assert!(
            !matches.contains(&(4, 6)),
            "Should not find match at original position"
        );
    }

    #[test]
    fn test_regex_matcher_offset_multiple_matches() {
        let offset = 5;
        let re1 = vec![regex::bytes::Regex::new("A").unwrap()];
        let mut matcher = RegexMatcher::new(re1, vec![], vec![], offset);

        let sequence = b"GAAATTAAGG";
        let mut matches = HashSet::new();

        matcher.match_primary(sequence, &mut matches, true);

        // Original positions: 1, 2, 3, 6, 7
        // With offset=5: 6, 7, 8, 11, 12
        let expected_matches = vec![(6, 7), (7, 8), (8, 9), (11, 12), (12, 13)];

        for expected in expected_matches {
            assert!(
                matches.contains(&expected),
                "Should find match at offset position {:?}",
                expected
            );
        }

        // Should have exactly 5 matches
        assert_eq!(matches.len(), 5, "Should find exactly 5 matches");
    }

    #[test]
    fn test_regex_matcher_offset_secondary() {
        let offset = 7;
        let re2 = vec![regex::bytes::Regex::new("TT").unwrap()];
        let mut matcher = RegexMatcher::new(vec![], re2, vec![], offset);

        let sequence = b"AAAATTGGTT";
        let mut matches = HashSet::new();

        matcher.match_secondary(sequence, &mut matches, true);

        // Original positions: (4, 6) and (8, 10)
        // With offset=7: (11, 13) and (15, 17)
        assert!(
            matches.contains(&(11, 13)),
            "Should find first match at offset position"
        );
        assert!(
            matches.contains(&(15, 17)),
            "Should find second match at offset position"
        );
    }

    #[test]
    fn test_regex_matcher_offset_either() {
        let offset = 3;
        let re = vec![regex::bytes::Regex::new("CC").unwrap()];
        let mut matcher = RegexMatcher::new(vec![], vec![], re, offset);

        let primary = b"GGGGAAAATTTT";
        let secondary = b"GGGGCCCCTTTT";
        let mut smatches = HashSet::new();
        let mut xmatches = HashSet::new();

        matcher.match_either(primary, secondary, &mut smatches, &mut xmatches, true);

        // Should not match in primary
        assert!(smatches.is_empty(), "Should not match in primary");

        // Should match in secondary - CC pattern will find overlapping matches
        assert!(!xmatches.is_empty(), "Should find matches in secondary");

        // Verify all matches have the offset applied
        for (start, end) in &xmatches {
            assert!(
                *start >= offset,
                "Match start should include offset, found: {:?}",
                xmatches
            );
            assert!(
                *end > offset,
                "Match end should be greater than offset, found: {:?}",
                xmatches
            );
            assert!(
                *end - *start == 2,
                "CC pattern should match exactly 2 characters"
            );
        }
    }

    #[cfg(feature = "fuzzy")]
    #[test]
    fn test_fuzzy_matcher_offset_zero() {
        let pat1 = vec![b"AAAA".to_vec()];
        let mut matcher = FuzzyMatcher::new(pat1, vec![], vec![], 0, false, 0);

        let sequence = b"GGGGAAAAATTTT";
        let mut matches = HashSet::new();

        matcher.match_primary(sequence, &mut matches, true);

        // With offset=0, should find match at original position
        assert!(!matches.is_empty(), "Should find at least one match");
        // Verify that with offset=0, no additional offset is applied
        // The fuzzy matcher found a match, which is the actual match position
        assert_eq!(matcher.offset(), 0);
    }

    #[cfg(feature = "fuzzy")]
    #[test]
    fn test_fuzzy_matcher_offset_nonzero() {
        let offset = 15;
        let pat1 = vec![b"AAAA".to_vec()];
        let mut matcher = FuzzyMatcher::new(pat1.clone(), vec![], vec![], 1, false, offset);

        let sequence = b"GGGGAAAAATTTT";
        let mut matches = HashSet::new();

        matcher.match_primary(sequence, &mut matches, true);

        // With offset=15, positions should be adjusted
        assert!(!matches.is_empty(), "Should find at least one match");

        // Create a matcher with offset=0 to get the baseline positions
        let mut baseline_matcher = FuzzyMatcher::new(pat1.clone(), vec![], vec![], 1, false, 0);
        let mut baseline_matches = HashSet::new();
        baseline_matcher.match_primary(sequence, &mut baseline_matches, true);
        let baseline_match = baseline_matches.iter().next().unwrap();

        // With offset, all matches should be shifted by the offset amount
        for (start, end) in &matches {
            assert_eq!(
                *start,
                baseline_match.0 + offset,
                "Start should be baseline + offset"
            );
            assert_eq!(
                *end,
                baseline_match.1 + offset,
                "End should be baseline + offset"
            );
        }
        assert_eq!(matcher.offset(), offset);

        // Should NOT contain the baseline positions (without offset)
        for baseline_match in &baseline_matches {
            assert!(
                !matches.contains(baseline_match),
                "Should not find match at baseline position {:?}",
                baseline_match
            );
        }
    }

    #[cfg(feature = "fuzzy")]
    #[test]
    fn test_fuzzy_matcher_offset_with_mismatch() {
        let offset = 8;
        let pat1 = vec![b"AAAA".to_vec()];
        let mut matcher = FuzzyMatcher::new(pat1, vec![], vec![], 1, false, offset);

        // One mismatch in the pattern
        let sequence = b"GGGGAACAATTTT";
        let mut matches = HashSet::new();

        matcher.match_primary(sequence, &mut matches, true);

        // Should find fuzzy match with offset
        assert!(!matches.is_empty(), "Should find fuzzy match with offset");

        // Verify all matches have the offset applied
        for (start, end) in &matches {
            assert!(*start >= offset, "Match start should include offset");
            assert!(*end > offset, "Match end should be greater than offset");
        }
    }

    #[cfg(feature = "fuzzy")]
    #[test]
    fn test_fuzzy_matcher_offset_secondary() {
        let offset = 12;
        let pat2 = vec![b"TTTT".to_vec()];
        let mut matcher = FuzzyMatcher::new(vec![], pat2, vec![], 1, false, offset);

        let sequence = b"GGGGTTTTCCCC";
        let mut matches = HashSet::new();

        matcher.match_secondary(sequence, &mut matches, true);

        // Original match at (4,8), with offset=12 should be (16,20)
        assert!(
            matches.contains(&(16, 20)),
            "Should find match at offset position in secondary"
        );
    }

    #[test]
    fn test_offset_consistency_across_methods() {
        let offset = 25;
        let re1 = vec![regex::bytes::Regex::new("AA").unwrap()];
        let re2 = vec![regex::bytes::Regex::new("TT").unwrap()];
        let re = vec![regex::bytes::Regex::new("GG").unwrap()];

        let mut matcher = RegexMatcher::new(re1, re2, re, offset);

        // Test that offset is consistent across all methods
        assert_eq!(matcher.offset(), offset, "Offset should be consistent");

        let sequence = b"AATTGGCCAAGG";
        let mut matches1 = HashSet::new();
        let mut matches2 = HashSet::new();
        let mut smatches = HashSet::new();
        let mut xmatches = HashSet::new();

        matcher.match_primary(sequence, &mut matches1, false);
        matcher.match_secondary(sequence, &mut matches2, false);
        matcher.match_either(sequence, sequence, &mut smatches, &mut xmatches, false);

        // All matches should have positions adjusted by the same offset
        let all_matches = matches1
            .union(&matches2)
            .chain(smatches.iter())
            .chain(xmatches.iter())
            .collect::<Vec<_>>();

        for (start, end) in all_matches {
            assert!(*start >= offset, "All match starts should include offset");
            assert!(
                *end > offset,
                "All match ends should be greater than offset"
            );
        }
    }

    #[test]
    fn test_large_offset_edge_case() {
        let offset = 1000;
        let re1 = vec![regex::bytes::Regex::new("A").unwrap()];
        let mut matcher = RegexMatcher::new(re1, vec![], vec![], offset);

        let sequence = b"AAAA";
        let mut matches = HashSet::new();

        matcher.match_primary(sequence, &mut matches, false);

        // Even with large offset, should still find matches
        assert!(
            !matches.is_empty(),
            "Should find matches even with large offset"
        );

        // All positions should be offset by 1000
        for (start, end) in &matches {
            assert!(*start >= 1000, "Start position should include large offset");
            assert!(*end >= 1001, "End position should include large offset");
            assert!(*end - *start == 1, "Match length should remain 1");
        }
    }

    #[test]
    fn test_complex_pattern_combinations() {
        // Test combining primary, secondary, and either patterns
        let re1 = vec![regex::bytes::Regex::new("AAAA").unwrap()];
        let re2 = vec![regex::bytes::Regex::new("TTTT").unwrap()];
        let re = vec![regex::bytes::Regex::new("CCCC").unwrap()];

        let mut matcher = RegexMatcher::new(re1, re2, re, 0);

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
