# bqtools

[![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE.md)
[![Crates.io](https://img.shields.io/crates/d/bqtools?color=orange&label=crates.io)](https://crates.io/crates/bqtools)

A command-line utility for working with BINSEQ files.

## Overview

bqtools provides tools to encode, decode, manipulate, and analyze [BINSEQ](https://github.com/arcinstitute/binseq) files.
It supports both (`*.bq`) and (`*.vbq`) files and makes use of the [`binseq`](https://crates.io/crates/binseq) library.

BINSEQ is a binary file format family designed for high-performance processing of DNA sequences.
It currently has two variants: BQ and VBQ.

- **BQ (\*.bq)**: Optimized for _fixed-length_ DNA sequences **without** quality scores.
- **VBQ (\*.vbq)**: Optimized for _variable-length_ DNA sequences **with optional** quality scores.

Both support single and paired sequences and make use of two-bit or four-bit encoding for efficient nucleotide packing using [`bitnuc`](https://crates.io/crates/bitnuc) and efficient parallel FASTX processing using [`paraseq`](https://crates.io/crates/paraseq).

For more information about BINSEQ, see our [preprint](https://www.biorxiv.org/content/10.1101/2025.04.08.647863v1) where we describe the format family and its applications.

## Features

- **Encode**: Convert FASTA or FASTQ files to a BINSEQ format
- **Decode**: Convert a BINSEQ file back to FASTA, FASTQ, or TSV format
- **Cat**: Concatenate multiple BINSEQ files
- **Count**: Count records in a BINSEQ file
- **Grep**: Search for fixed-string, regex, or fuzzy matches in BINSEQ files.
- **Pipe**: Create named-pipes for efficient data processing with legacy tools that don't support BINSEQ.

## Installation

### From Cargo

bqtools can be installed using `cargo`, the Rust package manager:

```bash
cargo install bqtools
```

To install `cargo` you can follow the instructions on the [official Rust website](https://www.rust-lang.org/tools/install).

### From Source

```bash
# Clone the repository
git clone https://github.com/arcinstitute/bqtools.git
cd bqtools

# Install
cargo install --path .

# Check installation
bqtools --help
```

### Feature Flags

bqtools supports the following feature flags:

- `htslib`: Enable support for reading SAM/BAM/CRAM files using the [`htslib`](https://docs.rs/rust-htslib/latest/rust_htslib/) library (default).
- `gcs`: Enable support for reading Google Cloud Storage files (default).
- `fuzzy`: Enable fuzzy matching in the `grep` command using the [`sassy`](https://crates.io/crates/sassy) library

To enable fuzzy matching, `bqtools` must be compiled using a `native` target cpu:

```bash
# Install from source
cargo install --path . -F fuzzy;

# Or install from crates but enforce native target cpu
export RUSTFLAGS="-C target-cpu=native"; cargo install bqtools -F fuzzy;
```

To selectively enable/disable feature flags:

```bash
# (for fuzzy matching support sassy requires native target cpu)
export RUSTFLAGS="-C target-cpu=native";

# Install bqtools without htslib/gcs but with fuzzy matching
cargo install bqtools --no-default-features -F fuzzy
# 
# Install bqtools without htslib but with fuzzy matching and gcs
cargo install bqtools --no-default-features -F fuzzy,gcs
```


## Usage

```bash
# Get help information
bqtools --help

# Get help for specific commands
bqtools encode --help
bqtools decode --help
bqtools cat --help
bqtools count --help
```

### Encoding

`bqtools` accepts input from stdin or from file paths.

It will auto-determine the input format and compression status.

Convert FASTA/FASTQ files to BINSEQ:

```bash
# Encode a single file to bq
bqtools encode input.fastq -o output.bq

# Encode a single file to vbq
bqtools encode input.fastq -o output.vbq

# Encode a single file to vbq with 4bit encoding
bqtools encode input.fastq -o output.vbq -S4

# Encode a file stream to bq (auto-determine input format and compression status)
/bin/cat input.fastq.zst | bqtools encode -o output.bq

# Encode paired-end reads
bqtools encode input_R1.fastq input_R2.fastq -o output.bq

# Encode paired-end reads to vbq
bqtools encode input_R1.fastq input_R2.fastq -o output.vbq

# Encode a SAM/BAM/CRAM file to BINSEQ
bqtools encode input.bam -fb -o output.bq

# Encode an paired-end CRAM file to BINSEQ (sorted by read name)
bqtools encode input.paired.cram -I -fb -o output.vbq

# Specify a policy for handling non-ATCG nucleotides (2-bit only)
bqtools encode input.fastq -o output.bq -p r  # Randomly draw A/C/G/T for each N

# Set threads for parallel processing
bqtools encode input.fastq -o output.bq -T 4

# Include sequencing headers in the encoding (unused by .bq)
bqtools encode input.fastq -o output.vbq -H

# Encode with ARCHIVE mode (useful for genomes, cDNA libraries, and larger sequences)
# where there are common Ns, large sequence sizes, and headers are important
bqtools encode input.fasta -o output.vbq -A
```

Available policies for handling non-ATCG nucleotides:

- `i`: Ignore sequences with non-ATCG characters
- `p`: Break on invalid sequences
- `r`: Randomly draw a nucleotide for each N (default)
- `a`: Set all Ns to A
- `c`: Set all Ns to C
- `g`: Set all Ns to G
- `t`: Set all Ns to T

> Note: These are only applied when encoding with 2-bit.

#### Recursive Encoding

You might have a directory or nested subdirectories with multiple FASTX files or FASTX file pairs.

`bqtools` makes use of the efficient [`walkdir`](https://crates.io/crates/walkdir) crate to recursively identify all FASTX files with various compression formats.
It will then balance the provided file/file pairs among the thread pool to ensure efficient parallel encoding.

All options provided by `bqtools encode` will be passed through to the sub-encoders.

```bash
# Encode all FASTX files as BQ
bqtools encode --recursive --mode bq ./

# Encode all paired FASTX files as VBQ and index their output
bqtools encode --recursive --paired --mode vbq --index ./

# Encode recursively with a max-subdirectory depth of 2
bqtools encode --recursive --mode bq --depth 2 ./
```

### Decoding

Convert BINSEQ files back to FASTA/FASTQ/TSV:

```bash
# Decode to FASTQ (default)
bqtools decode input.bq -o output.fastq

# Decode to compressed FASTQ (gzip/zstd)
bqtools decode input.bq -o output.fastq.gz
bqtools decode input.bq -o output.fastq.zst

# Decode to FASTA
bqtools decode input.bq -o output.fa -f a

# Decode paired-end reads into separate files
bqtools decode input.bq --prefix output
# Creates output_R1.fastq and output_R2.fastq

# Specify which read of a pair to output
bqtools decode input.bq -o output.fastq -m 1  # Only first read
bqtools decode input.bq -o output.fastq -m 2  # Only second read

# Specify output format
bqtools decode input.bq -o output.tsv -f t  # TSV format
```

### Concatenating

Combine multiple BINSEQ files:

```bash
bqtools cat file1.bq file2.bq file3.bq -o combined.bq
```

### Counting

Count records in a BINSEQ file:

```bash
bqtools count input.bq
```

### Grep

You can easily search for specific subsequences or regular expressions within BINSEQ files:

By default the multiple pattern logic is AND (i.e. all patterns must match).
The logic can be changed to OR (i.e. any pattern must match) with the `--or-logic` option.

```bash
# See full options list
bqtools grep --help

# Search for a specific regex in either sequence
bqtools grep input.bq "ACGT[AC]TCCA"

# Search for a specific subsequence (in primary sequence)
bqtools grep input.bq -r "ATCG"

# Search for a regular expression (in extended)
bqtools grep input.bq -R "AT[CG]"

# Search for multiple regular expressions in either
bqtools grep input.bq "ACGT[AG]TCCA" "AG(TTTT|CCCC)A"

# Search for multiple regular expressions (OR-logic)
bqtools grep input.bq "ACGT[AG]TCCA" "AG(TTTT|CCCC)A" --or-logic

# Only search for patterns within a specified range per sequence (basepairs 30-80)
bqtools grep input.bq "ACGT[AG]TCCA" --range 30..80

# Only search for patterns within a specified range per sequence (basepairs 0-80)
bqtools grep input.bq "ACGT[AG]TCCA" --range ..80

# Only search for patterns within a specified range per sequence (basepairs 80-max)
bqtools grep input.bq "ACGT[AG]TCCA" --range 80..
```

`bqtools` also support fuzzy matching by making use of [`sassy`](https://github.com/RagnarGrootKoerkamp/sassy).

This requires installing using the `fuzzy` feature flag (see installation above):

```bash
# Run grep with fuzzy matching (-z)
bqtools grep input.bq "ACGTACGT" -z

# Run fuzzy matching with an edit distance of 2
bqtools grep input.bq "ACGTACGT" -z -k2

# Run fuzzy matching but only write inexact matches
bqtools grep input.bq "ACGTACGT" -zi
```

`bqtools` can also handle a large collection of patterns which can be provided on the CLI as a file.
You can provide files for either primary/extended, just primary, or just extended patterns with the relevant flags.
Notably this will match *solely* with OR logic.
This can be used also with fuzzy matching as well as with pattern counting described below.
Regex is also fully supported and files can be additionally paired with CLI arguments.

If your patterns are all fixed strings (and not regex), you can improve performance by using the `-x/--fixed` flag.
This will use the more efficient [Aho-Corasick algorithm](https://en.wikipedia.org/wiki/Aho%E2%80%93Corasick_algorithm) to match patterns.

```bash
# Run grep with patterns from a file
bqtools grep input.bq --file patterns.txt

# Run grep with patterns from a file (primary)
bqtools grep input.bq --sfile patterns.txt

# Run grep with patterns from a file (extended)
bqtools grep input.bq --xfile patterns.txt

# Run grep with fixed-string patterns from a file
bqtools grep input.bq --file patterns.txt -x
```

`bqtools` also introduces a new feature for the counting the occurrences of individual patterns.
This is useful for seeing how many times each pattern occurs across a sequencing dataset without having to iterate over the dataset multiple times using traditional methods.

Some important notes are:
1. A pattern will only be counted once across a sequencing record (primary and secondary)
2. A sequencing record may contribute to multiple patterns occurrences
3. Providing multiple patterns will match records with `OR` logic (this is different behavior from `bqtools grep` default which uses `AND` logic when multiple patterns are provided)
4. Regular expressions are supported and treated as a single pattern (e.g. `ACGT|TCGA` will return a single output row but match on both `ACGT` and `TCGA`).
5. Invert is supported for counting patterns and will return the number of records a pattern does not occur in.

If your patterns are all fixed strings (and not regex), you can improve performance by using the `-x/--fixed` flag.
This will use the more efficient [Aho-Corasick algorithm](https://en.wikipedia.org/wiki/Aho%E2%80%93Corasick_algorithm) to match patterns.

The throughput gains for this can be massive for pattern counting, especially when dealing with high numbers of patterns.

```bash
# Count the number of occurrences for each of three expressions
bqtools grep input.bq "ACGTACGT" "TCGATCGA$" "AAA(TT|CC)AAA" -P

# Count the number of occurrences for each of three patterns with fuzzy matching
bqtools grep input.bq "ACGTACGT" "TCGATCGA" "AAAAAAAA" -Pz

# Count the number of records a pattern does not occur in
bqtools grep input.bq "ACGTACGT" "TCGATCGA" "AAAAAAAA" -Pv

# Count the number of occurrences for each pattern from a file
bqtools grep input.bq --file patterns.txt -P

# Count the number of occurrences for each pattern from a file (fixed strings)
bqtools grep input.bq --file patterns.txt -Px
```

The output of pattern count is a TSV with three columns: [Pattern, Count, Fraction of Total]

### Pipe

Stream BINSEQ data to legacy tools through named pipes for parallel processing.

Because BINSEQ is a new format, many tools don't support it yet. 
`bqtools pipe` creates a server that splits a BINSEQ file into multiple named pipes,
enabling parallel processing with tools that expect FASTQ/FASTA files.

Importantly, if your tool supports multiple parallel threads (i.e. parallelizes input files), you can make use of this feature to significantly improve performance.

```bash
# Create 4 named pipes (8 files for paired-end data)
# Pipes: fifo_0.fq, fifo_1.fq, fifo_2.fq, fifo_3.fq
bqtools pipe input.vbq -p 4 -b fifo &

# Process in parallel with tools that don't support BINSEQ
ls fifo_*.fq | xargs -P 4 -I {} sh -c 'legacy-tool {} > {.}.out'
```

**Key features:**
- Each pipe streams a portion of the BINSEQ file **sequentially**
- No disk I/O for intermediate files - data flows through memory
- Automatic paired-end handling (`_R1`/`_R2` pairs)
- Blocks until all pipes are fully read (prevents data loss)
- Auto-scales to CPU count with `-p0` (default)
- Pipes can be read sequentially *or* in parallel without blocking.

Note: This feature is not available on Windows.
