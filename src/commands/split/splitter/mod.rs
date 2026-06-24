mod ac_splitter;
mod processor;
mod regex_splitter;

pub use ac_splitter::AhoCorasickSplitter;
pub use processor::SplitProcessor;
pub use regex_splitter::RegexSplitter;

/// Resolves a record's (primary, secondary) sequences to a single output bin.
///
/// Backends implement the actual matching strategy (fixed-string, regex, fuzzy, …)
/// and expose the unique set of aliases that records can be split into.
pub trait SequenceSplit: Clone + Send + Sync {
    /// Returns the bin index a record belongs to, or `None` when the record's
    /// matches do not resolve to exactly one unique alias.
    fn split_idx(&mut self, primary: &[u8], secondary: &[u8]) -> Option<usize>;

    /// The unique aliases records can be split into, ordered by bin index.
    fn aliases(&self) -> &[String];
}

/// Dispatches across the available splitter backends.
#[derive(Clone)]
pub enum Splitter {
    AhoCorasick(AhoCorasickSplitter),
    Regex(RegexSplitter),
}
impl SequenceSplit for Splitter {
    fn split_idx(&mut self, primary: &[u8], secondary: &[u8]) -> Option<usize> {
        match self {
            Splitter::AhoCorasick(splitter) => splitter.split_idx(primary, secondary),
            Splitter::Regex(splitter) => splitter.split_idx(primary, secondary),
        }
    }

    fn aliases(&self) -> &[String] {
        match self {
            Splitter::AhoCorasick(splitter) => splitter.aliases(),
            Splitter::Regex(splitter) => splitter.aliases(),
        }
    }
}
