use super::PerBaseSequenceQuality;
use binseq::ParallelProcessor;

/// TODO: per base sequence quality
/// TODO: per sequence quality
/// TODO: per base sequence content
/// TODO: per sequence GC content
/// TODO: per base N content
/// TODO: sequence length distribution
/// TODO: sequence duplication levels
/// TODO: overrepresented sequences
/// TODO: adapter content
#[derive(Clone, Default)]
pub struct QcProcessor {
    pub bsq: PerBaseSequenceQuality,
}
impl QcProcessor {}
impl ParallelProcessor for QcProcessor {
    fn process_record<R: binseq::prelude::BinseqRecord>(
        &mut self,
        record: R,
    ) -> binseq::Result<()> {
        self.bsq.push(&record);
        Ok(())
    }

    fn on_batch_complete(&mut self) -> binseq::Result<()> {
        self.bsq.sync();
        Ok(())
    }
}
