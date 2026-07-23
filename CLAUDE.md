# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

bqtools is a Rust CLI for working with BINSEQ files — a binary format family for high-performance DNA sequence processing. It encodes, decodes, greps, concatenates, samples, pipes, and runs QC on BINSEQ files (`.bq`, `.vbq`, `.cbq`). CBQ is the recommended format for most applications.

## Build & Test Commands

```bash
cargo build                    # Debug build
cargo build --release          # Optimized build (uses LTO, slow)
cargo install --path .         # Install binary locally

cargo test --verbose           # Run all tests
cargo test --verbose -F fuzzy  # Run tests including fuzzy feature
cargo test <test_name>         # Run a single test by name

cargo fmt --check              # Check formatting
cargo clippy --verbose         # Lint (pedantic clippy enabled)
```

Logging is controlled via `BQTOOLS_LOG` environment variable (uses `env_logger`).

## Feature Flags

- `htslib` (default): SAM/BAM/CRAM support via rust-htslib
- `gcs` (default): Google Cloud Storage file reading
- `fuzzy` (optional): Fuzzy matching via `sassy` — requires `RUSTFLAGS="-C target-cpu=native"`

Build without defaults: `cargo build --no-default-features -F fuzzy,gcs`

## Architecture

### Module Layout

- **`src/cli/`** — Clap derive-based argument definitions. `cli.rs` has the top-level `Commands` enum. `input.rs` and `output.rs` handle complex input/output argument parsing (file formats, compression, paired-end, spans).
- **`src/commands/`** — Command implementations, each in its own subdirectory. `utils.rs` has shared compression helpers.
- **`src/types.rs`** — Type aliases (`BoxedReader`, `BoxedWriter`).
- **`src/main.rs`** — CLI dispatch and SIGPIPE handling.

### Key Patterns

**Parallel processing**: Commands use the `paraseq` crate's `ParallelProcessor` trait for embarrassingly parallel batch processing. Each command has a `processor.rs` implementing this trait with thread-local buffers and `Arc<Mutex<T>>` for shared global state.

**Grep backends**: The grep command uses a `PatternMatcher` enum dispatching to three backends — `regex`, `aho-corasick` (fixed-string, multi-pattern), and `sassy` (fuzzy, feature-gated). The same pattern applies to `PatternCounter` for the `-P` pattern-count mode. All backends accept `PatternCollection` which carries optional pattern names (from FASTA headers).

**Pattern types**: `patterns.rs` defines `Pattern` (name + sequence) and `PatternCollection` (newtype over `Vec<Pattern>`) with methods `.bytes()`, `.regexes()`, `.names()`. Pattern files (`--file`, `--sfile`, `--xfile`) auto-detect FASTA vs plain text. FASTA headers become pattern names; plain text patterns have no name and fall back to the pattern string in output.

**Encode modes**: Encoding dispatches across atomic (single/paired files), recursive (directory walk via `walkdir`), manifest (file list), and batch (multi-file thread distribution) modes.

**Writer abstraction**: `SplitWriter` supports interleaved (single file) and split (separate R1/R2) output modes with polymorphic writers (file, stdout, compressed, chunked).

**Pipe exec modes**: The pipe command (`src/commands/pipe/`) splits a BINSEQ file across named FIFOs (one writer thread per pipe). It can optionally spawn and supervise the consumer processes via `ExecMode` (`exec.rs`): `PerFifo` (`-x`/`--exec`) runs one shell command per pipe, while `Batch` (`-X`/`--exec-batch`) runs a single command with all FIFO paths space-joined. Templates use `{}` (single-end), `{R1}`/`{R2}` (paired-end), and `{n}` (pipe index, `-x` only). Templates are validated up front so a malformed template fails before any FIFO is opened (an unread FIFO would hang). `PairedChannels` (`mod.rs`) is derived from the template's tokens so referencing only `{R1}` or `{R2}` suppresses the unused channel's FIFOs and writer threads entirely. Consumers must be spawned before writer threads open the FIFOs, since opening a FIFO for writing blocks until a reader connects.

**QC modules**: The qc command (`src/commands/qc/`) runs a FastQC-style suite of independent modules (per-base quality, per-sequence quality, per-base content, per-sequence GC content, sequence length distribution, sequence duplication levels, overrepresented sequences) behind the `QcModule` trait, dispatched through a `QcModuleType` enum (`modules.rs`). Each module implements `push` (per-record), `sync_batch`/`sync_final` (thread-local → shared merge), `finish` (writes its own `<name>_R1.tsv`/`_R2.tsv`), and an optional `summarize` (renders its headline stats into the shared `summary.md`, built via `report.rs`'s `table`/`dual_section` helpers). `QcConfig` (`config.rs`) turns `--skip-*` CLI flags into the enabled module list; duplication-level and overrepresented-sequence estimation only sample the first `--dup-sample-size` records.

**Order-independent checksums**: The verify command (`src/commands/verify/`) hashes each record with `xxh3-64` (`processor.rs`) over the user-selected fields (`--skip-seq`/`--skip-qual`/`--skip-headers`/`--skip-flags`, and `-M/--mate` for paired files) and combines per-record hashes with a wrapping sum — a commutative operation — so the resulting checksum is identical regardless of record order. This matters because parallel BINSEQ writers make no guarantee that output record order matches input order. Each field is length-prefixed before hashing so adjacent fields can't be confused for one another at their boundary. A field is only hashed when the file actually carries that data, gated on file-level presence (`record.has_quality()` for quality; `mod.rs`'s `reader_has_headers()`, checked once against the reader, for headers) or per-record presence (`record.flag().is_some()` for flags) rather than on the `--skip-*` flag alone — otherwise toggling `--skip-*` would change the checksum on files that never had that data, and worse, for headers specifically, `BinseqRecord::sheader`/`xheader` fall back to a string synthesized from the record's position when a file has no real headers (bq/vbq/cbq all do this, for use by commands like `decode` that need some name to print), so hashing it unconditionally would leak record order into a checksum that's supposed to be order-independent. `-M 2` on a single-channel file hard-errors instead of silently hashing nothing, since (unlike headers/flags/quality) there's no reasonable no-op fallback for "the mate the user explicitly asked for doesn't exist". Lengths and flag values are fed into the hasher via explicit `to_le_bytes()`, not `Hasher::write_u64` — that trait method's default implementation serializes via `to_ne_bytes()`, which would make the checksum depend on the host's endianness (identical file, different byte order fed to the hasher, different checksum on a big-endian host) if left unfixed.

### Core Dependencies

| Crate     | Role                             |
| --------- | -------------------------------- |
| `binseq`  | BINSEQ format read/write         |
| `bitnuc`  | 2-bit/4-bit nucleotide encoding  |
| `paraseq` | Parallel FASTX/BINSEQ processing |
| `clap`    | CLI argument parsing (derive)    |
| `anyhow`  | Error handling throughout        |

### Testing

Integration tests live in `tests/`. `tests/common.rs` provides a builder (`write_fastx()`) for generating random FASTQ/FASTA test data with configurable compression (none, gzip, zstd). Tests use cartesian products over format/compression/mode combinations. Dev dependencies: `bon` (builder macro), `nucgen` (random sequences), `tempfile`, `itertools`.

### Generating Test Data

Random FASTQ/FASTA test data can be created on the CLI with `nucgen` (`cargo install nucgen` if not already installed).

```bash
# generates 10,000 reads of length 150
nucgen -n 10000 -l 150 some.fq
# generates 30,000 paired-reads of length 50 and 200
nucgen -n 30000 -l 50 -L 200 some_R1.fq some_R2.fq
```

These can then be ingested with `bqtools encode`:

```bash
bqtools encode some.fq -o some.cbq
bqtools encode some_R1.fq some_R2.fq -o some.cbq
```

### Benchmarking Changes

Make use of `hyperfine` (`cargo install hyperfine` if not already installed) to measure performance of binaries after changes.

```bash
# Measures decoding performance
hyperfine --warmup 3 --runs 10 "bqtools decode some.cbq > /dev/null"
```

## Contribution Guide

When making changes, keep the following documentation in sync:

1. **CLAUDE.md** — Update this file when adding new commands, changing architecture, or modifying build/test workflows.
2. **README.md** — Update usage examples and feature descriptions when adding or changing user-facing functionality (new commands, flags, behavior changes).
3. **Clap doc comments** — All CLI arguments, flags, and subcommands use clap derive macros with `/// doc comments` and `#[clap(long_about)]` attributes. When adding or modifying flags, write clear help text directly on the struct fields in `src/cli/`. These doc comments are the `--help` output users see.
4. **New feature flags** — If adding a Cargo feature flag, document it in both `CLAUDE.md` (Feature Flags section) and `README.md` (Feature Flags / Installation section).
