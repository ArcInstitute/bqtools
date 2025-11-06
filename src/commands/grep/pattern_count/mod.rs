mod regex_pc;
pub use regex_pc::RegexPatternCounter;

#[cfg(feature = "fuzzy")]
mod fuzzy_pc;
#[cfg(feature = "fuzzy")]
pub use fuzzy_pc::FuzzyPatternCounter;

mod processor;
pub use processor::PatternCountProcessor;

pub trait PatternCount: Clone + Send + Sync {
    fn count_patterns(
        &mut self,
        primary: &Vec<u8>,
        secondary: &Vec<u8>,
        pattern_count: &mut [usize],
    );

    fn num_patterns(&self) -> usize;

    fn pattern_strings(&self) -> Vec<String>;
}
