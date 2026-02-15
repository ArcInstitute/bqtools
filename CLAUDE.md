# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

For **using** bqtools as a tool (encoding, decoding, grepping, etc.), see `skills/bqtools/SKILL.md` instead. This file is about **developing and contributing to** the bqtools codebase.

## Project Overview

bqtools is a Rust CLI for working with BINSEQ files — a binary format family for high-performance DNA sequence processing. It supports three format variants (`.bq`, `.vbq`, `.cbq`) with CBQ as the recommended default.

Repository: https://github.com/arcinstitute/bqtools

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

**Grep backends**: The grep command uses a `PatternMatcher` enum dispatching to three backends — `regex`, `aho-corasick` (fixed-string, multi-pattern), and `sassy` (fuzzy, feature-gated). The same pattern applies to `PatternCounter` for the `-P` pattern-count mode.

**Encode modes**: Encoding dispatches across atomic (single/paired files), recursive (directory walk via `walkdir`), manifest (file list), and batch (multi-file thread distribution) modes.

**Writer abstraction**: `SplitWriter` supports interleaved (single file) and split (separate R1/R2) output modes with polymorphic writers (file, stdout, compressed, chunked).

### Core Dependencies

| Crate     | Role                             |
| --------- | -------------------------------- |
| `binseq`  | BINSEQ format read/write         |
| `bitnuc`  | 2-bit/4-bit nucleotide encoding  |
| `paraseq` | Parallel FASTX/BINSEQ processing |
| `clap`    | CLI argument parsing (derive)    |
| `anyhow`  | Error handling throughout        |

## Testing

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
2. **skills/bqtools/SKILL.md** — Update the usage skill when adding or changing user-facing functionality (new commands, flags, behavior changes, new recipes).
3. **README.md** — Update usage examples and feature descriptions for user-facing changes.
4. **Clap doc comments** — All CLI arguments, flags, and subcommands use clap derive macros with `/// doc comments` and `#[clap(long_about)]` attributes. When adding or modifying flags, write clear help text directly on the struct fields in `src/cli/`. These doc comments are the `--help` output users see.
5. **New feature flags** — If adding a Cargo feature flag, document it in CLAUDE.md (Feature Flags section), README.md, and skills/bqtools/SKILL.md if it affects user-facing behavior.
