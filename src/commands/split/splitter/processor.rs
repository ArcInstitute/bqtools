use std::{
    io::{stderr, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Result;
use binseq::{BinseqWriter, BinseqWriterBuilder, ParallelProcessor, SequencingRecordBuilder};
use parking_lot::Mutex;

use crate::{
    cli::BinseqMode,
    commands::{
        match_output,
        split::splitter::{SequenceSplit, Splitter},
    },
    types::BoxedWriter,
};

#[derive(Clone)]
pub struct SplitProcessor {
    /// Thread-local matcher
    matcher: Splitter,

    /// Whether the undetermined writer is active
    write_undetermined: bool,

    /// Thread-local writers for the split processor.
    t_writer: Vec<BinseqWriter<Vec<u8>>>,
    t_counts: Vec<usize>,

    /// Global writers for the split processor.
    writer: Arc<Vec<Mutex<BinseqWriter<BoxedWriter>>>>,
    counts: Arc<Vec<Mutex<usize>>>,

    /// Aliases for each output bin (excludes the undetermined writer).
    aliases: Vec<String>,

    /// Output file path for each writer (parallel to `writer`/`counts`).
    paths: Vec<PathBuf>,
}
impl SplitProcessor {
    pub fn new<P: AsRef<Path>>(
        matcher: Splitter,
        builder: &BinseqWriterBuilder,
        output_basepath: P,
        output_mode: BinseqMode,
        write_undetermined: bool,
        undetermined_basepath: &str,
    ) -> Result<Self> {
        let mut t_writer = Vec::default();
        let mut writer = Vec::default();
        let mut paths = Vec::default();

        let mut extend_writers = |basename| -> Result<()> {
            let output_path =
                output_basepath
                    .as_ref()
                    .join(format!("{}{}", basename, output_mode.extension()));
            let output_handle = match_output(Some(output_path.clone()))?;

            let gw = builder.clone().build(output_handle)?;
            let tw = gw.new_headless_buffer()?;

            t_writer.push(tw);
            writer.push(Mutex::new(gw));
            paths.push(output_path);

            Ok(())
        };

        let aliases = matcher.aliases().to_vec();
        aliases
            .iter()
            .map(String::as_str)
            .try_for_each(&mut extend_writers)?;

        if write_undetermined {
            extend_writers(undetermined_basepath)?;
        }

        let t_counts = vec![0; t_writer.len()];
        let counts = Arc::new((0..writer.len()).map(|_| Mutex::new(0)).collect::<Vec<_>>());

        Ok(Self {
            matcher,
            write_undetermined,
            t_writer,
            t_counts,
            writer: Arc::new(writer),
            counts,
            aliases,
            paths,
        })
    }

    pub fn finish(&mut self) -> binseq::Result<()> {
        self.writer.iter().try_for_each(|w| w.lock().finish())
    }

    /// Removes any output files that received fewer than `min_records` records.
    ///
    /// Must be called after [`finish`](Self::finish) so all writers are flushed.
    /// Returns the number of files removed.
    pub fn prune_below(&self, min_records: usize) -> Result<usize> {
        let mut removed = 0;
        for (path, count) in self.paths.iter().zip(self.counts.iter().map(|c| *c.lock())) {
            if count < min_records {
                log::debug!(
                    "Removing {} ({count} records, below threshold of {min_records})",
                    path.display(),
                );
                std::fs::remove_file(path)?;
                removed += 1;
            }
        }
        Ok(removed)
    }

    pub fn pprint_counts(&self) -> Result<()> {
        let mut handle = stderr();
        self.aliases
            .iter()
            .zip(self.counts.iter().map(|x| *x.lock()))
            .try_for_each(|(alias, count)| writeln!(&mut handle, "{alias}\t{count}"))?;
        handle.flush().map_err(Into::into)
    }
}
impl ParallelProcessor for SplitProcessor {
    fn process_record<R: binseq::prelude::BinseqRecord>(
        &mut self,
        record: R,
    ) -> binseq::Result<()> {
        let sseq = record.sseq();
        let xseq = record.xseq();
        let rec = if record.is_paired() {
            SequencingRecordBuilder::default()
                .s_seq(record.sseq())
                .opt_s_qual(record.has_quality().then(|| record.squal()))
                .s_header(record.sheader())
                .x_seq(record.xseq())
                .opt_x_qual(record.has_quality().then(|| record.xqual()))
                .x_header(record.xheader())
                .build()?
        } else {
            SequencingRecordBuilder::default()
                .s_seq(record.sseq())
                .opt_s_qual(record.has_quality().then(|| record.squal()))
                .s_header(record.sheader())
                .build()?
        };
        if let Some(pattern_idx) = self.matcher.split_idx(sseq, xseq) {
            // handle match
            self.t_writer
                .get_mut(pattern_idx)
                .map(|w| -> binseq::Result<()> {
                    w.push(rec)?;
                    Ok(())
                });

            if let Some(c) = self.t_counts.get_mut(pattern_idx) {
                *c += 1;
            }
        } else if self.write_undetermined {
            // always the last writer
            let undetermined_idx = self.t_writer.len() - 1;

            // handle match
            self.t_writer
                .get_mut(undetermined_idx)
                .expect("number of writers misconfigured (undetermined)")
                .push(rec)?;

            if let Some(c) = self.t_counts.get_mut(undetermined_idx) {
                *c += 1;
            }
        }
        Ok(())
    }

    fn on_batch_complete(&mut self) -> binseq::Result<()> {
        // ingest counts
        self.counts.iter().zip(self.t_counts.iter_mut()).for_each(
            |(global_counts, thread_counts)| {
                *global_counts.lock() += *thread_counts;
                *thread_counts = 0;
            },
        );

        // ingest reads
        self.writer
            .iter()
            .zip(self.t_writer.iter_mut())
            .try_for_each(|(global_writer, thread_writer)| {
                global_writer.lock().ingest(thread_writer)
            })
    }
}
