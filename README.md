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
- **Grep**: Search for fixed-string or regex patterns in BINSEQ files.

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

```bash
# See full options list
bqtools grep --help

# Search for a specific regex in either sequence
bqtools grep input.bq "ACGT[AC]TCCA"

# Search for a specific subsequence (in primary sequence)
bqtools grep input.bq -r "ATCG"

# Search for a regular expression (in extended)
bqtools grep input.bq -R "AT[CG]"
```
