# bqtools

A command-line utility for working with BINSEQ files.

## Overview

bqtools provides tools to encode, decode, concatenate, and analyze BINSEQ (.bq) files. BINSEQ is a binary format designed for efficient storage of fixed-length DNA sequences, using 2-bit encoding for nucleotides.

## Features

- **Encode**: Convert FASTA or FASTQ files to BINSEQ format
- **Decode**: Convert BINSEQ files back to FASTA, FASTQ, or TSV format
- **Cat**: Concatenate multiple BINSEQ files
- **Count**: Count records in a BINSEQ file

## Installation

### From Source

```bash
# Clone the repository
git clone https://github.com/noamteyssier/bqtools.git
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
# Encode a single file
bqtools encode input.fastq -o output.bq

# Encode paired-end reads
bqtools encode input_R1.fastq input_R2.fastq -o output.bq

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

## BINSEQ Format Specification

BINSEQ (.bq) is a binary file format designed for efficient storage of fixed-length DNA sequences.

### File Structure

A BINSEQ file consists of:

1. Fixed-size header (32 bytes)
2. Record data section

### Header Format (32 bytes total)

| Offset | Size (bytes) | Name     | Description                  | Type   |
| ------ | ------------ | -------- | ---------------------------- | ------ |
| 0      | 4            | magic    | Magic number (0x42534551)    | uint32 |
| 4      | 1            | format   | Format version (currently 1) | uint8  |
| 5      | 4            | slen     | Sequence length (primary)    | uint32 |
| 9      | 4            | xlen     | Sequence length (secondary)  | uint32 |
| 13     | 19           | reserved | Reserved for future use      | bytes  |

### Record Format

Each record consists of:

1. Flag field (8 bytes, uint64)
2. Sequence data (ceil(N/32) \* 8 bytes, where N is sequence length)

### Encoding

- Each nucleotide is encoded using 2 bits:
  - A = 00
  - C = 01
  - G = 10
  - T = 11
- Non-ATCG characters are unsupported by default (use policies to handle them)
- Sequences are stored in Little-Endian order
- The final u64 of sequence data is padded with zeros if the sequence length is not divisible by 32

## License

MIT License

## Author

Noam Teyssier
