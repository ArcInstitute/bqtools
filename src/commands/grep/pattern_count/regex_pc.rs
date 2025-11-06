use super::PatternCount;

type Expressions = Vec<regex::bytes::Regex>;
#[derive(Clone)]
pub struct RegexPatternCounter {
    /// Regex expressions to match on
    re1: Expressions, // in primary
    re2: Expressions, // in secondary
    re: Expressions,  // in either
    invert: bool,
}
impl RegexPatternCounter {
    pub fn new(re1: Expressions, re2: Expressions, re: Expressions, invert: bool) -> Self {
        RegexPatternCounter {
            re1,
            re2,
            re,
            invert,
        }
    }
    fn regex_primary(&self, sequence: &[u8], pattern_counts: &mut [usize]) {
        if self.re1.is_empty() {
            return;
        }
        self.re1.iter().enumerate().for_each(|(index, reg)| {
            if reg.find(sequence).is_some() != self.invert {
                pattern_counts[index] += 1;
            }
        });
    }

    fn regex_secondary(&self, sequence: &[u8], pattern_counts: &mut [usize]) {
        if self.re2.is_empty() || sequence.is_empty() {
            return;
        }
        self.re2.iter().enumerate().for_each(|(index, reg)| {
            if reg.find(sequence).is_some() != self.invert {
                pattern_counts[self.re1.len() + index] += 1;
            }
        });
    }

    fn regex_either(&self, primary: &[u8], secondary: &[u8], pattern_counts: &mut [usize]) {
        if self.re.is_empty() {
            return;
        }
        self.re.iter().enumerate().for_each(|(index, reg)| {
            if (reg.find(primary).is_some() || reg.find(secondary).is_some()) != self.invert {
                pattern_counts[self.re1.len() + self.re2.len() + index] += 1;
            }
        });
    }
}

impl PatternCount for RegexPatternCounter {
    fn count_patterns(
        &mut self,
        primary: &Vec<u8>,
        secondary: &Vec<u8>,
        pattern_count: &mut [usize],
    ) {
        self.regex_primary(primary, pattern_count);
        self.regex_secondary(secondary, pattern_count);
        self.regex_either(primary, secondary, pattern_count);
    }

    fn num_patterns(&self) -> usize {
        self.re1.len() + self.re2.len() + self.re.len()
    }

    fn pattern_strings(&self) -> Vec<String> {
        self.re1
            .iter()
            .chain(self.re2.iter())
            .chain(self.re.iter())
            .map(|reg| reg.to_string())
            .collect()
    }
}
