# bqtools

[![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE.md)
[![Crates.io](https://img.shields.io/crates/d/bqtools?color=orange&label=crates.io)](https://crates.io/crates/bqtools)

A command-line utility for working with BINSEQ files.

## Overview

bqtools provides tools to encode, decode, concatenate, and analyze [BINSEQ](https://github.com/arcinstitute/binseq) (.bq) and [VBINSEQ](https://github.com/arcinstitute/vbinseq) (.vbq) files.
BINSEQ is a binary format designed for efficient storage of fixed-length DNA sequences, using 2-bit encoding for nucleotides.
VBINSEQ is a binary format designed for efficient storage of variable-length DNA sequences with optional quality score support.

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

Convert FASTA/FASTQ files to BINSEQ format:

```bash
# Encode a single file to binseq
bqtools encode input.fastq -o output.bq

# Encode a single file to vbinseq
bqtools encode input.fastq -o output.vbq

# Encode paired-end reads
bqtools encode input_R1.fastq input_R2.fastq -o output.bq

# Encode paired-end reads to vbinseq
bqtools encode input_R1.fastq input_R2.fastq -o output.vbq

# Specify a policy for handling non-ATCG nucleotides
bqtools encode input.fastq -o output.bq -p r  # Randomly draw A/C/G/T for each N

# Use multiple threads for parallel processing
bqtools encode input.fastq -o output.bq -T 8
```

Available policies for handling non-ATCG nucleotides:

- `i`: Ignore sequences with non-ATCG characters
- `p`: Break on invalid sequences
- `r`: Randomly draw a nucleotide for each N (default)
- `a`: Set all Ns to A
- `c`: Set all Ns to C
- `g`: Set all Ns to G
- `t`: Set all Ns to T

_Note:_ Input FASTQ files may be compressed.

### Decoding

Convert BINSEQ files back to FASTA/FASTQ/TSV:

```bash
# Decode to FASTQ (default)
bqtools decode input.bq -o output.fastq

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

# Search for a specific subsequence (in primary sequence)
bqtools grep input.bq -e "ATCG"

# Search for a regular expression (in primary)
bqtools grep input.bq -r "AT[CG]"

# Search for both a subsequence (in extended sequence) and a regular expression (in either)
bqtools grep input.bq -E "ATCG" -P "AT[CG]"
```
