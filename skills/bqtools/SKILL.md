---
name: bqtools
description: >
  Use this skill whenever the user wants to work with BINSEQ files (.bq, .vbq, .cbq)
  or mentions bqtools. Also trigger when someone wants to: convert FASTQ/FASTA files to
  a more compact/faster format, search for DNA sequences or patterns in sequencing data,
  subsample sequencing reads, combine sequencing files, or feed sequencing data into
  legacy tools that only accept FASTQ. If someone mentions "BINSEQ", "binary sequence
  files", "bqtools", or wants faster alternatives to working with FASTQ/FASTA files
  directly, use this skill. This skill is also relevant when a user has .cbq, .bq, or
  .vbq files and wants to do anything with them — even if they don't know what the
  format is.
---

# bqtools

bqtools is a command-line tool that lets you work with DNA sequencing data in a fast,
compact binary format called BINSEQ. Think of it as a better container for the same
data that lives in FASTQ/FASTA files — smaller on disk, faster to process, and
searchable without decoding.

## What You Need to Know

**CBQ (`.cbq`) is the format to use.** Unless someone has a specific reason for
another format, always use `.cbq`. It's lossless (keeps quality scores, headers,
everything), handles variable-length reads, and compresses well.

The other formats exist but rarely matter:

- `.bq` — fastest but lossy (drops quality/headers, fixed-length only). Niche use.
- `.vbq` — older format, superseded by `.cbq`. Treat as legacy.

**bqtools auto-detects almost everything.** Input compression (gzip, zstd), file
format (FASTQ, FASTA, BAM), and paired-end pairing are all detected automatically.
You rarely need to specify input details.

### Is it lossless?

**CBQ: yes.** Encode → decode round-trips preserve headers, sequences, and quality
scores exactly. If you encode a FASTQ to CBQ and decode it back, you get the same
data. (Read _order_ may differ when using multiple threads, since batches are
processed in parallel.)

**BQ: no.** BQ drops quality scores and headers, and only supports fixed-length
reads. It also uses 2-bit encoding, which means non-ATCG bases (like N) must be
handled by a policy — they cannot be stored faithfully. Use BQ only when you
explicitly want sequence-only data and know your reads are fixed-length.

### How are Ns handled?

Real sequencing data often contains N bases. How bqtools handles them depends on
the encoding:

**CBQ** stores Ns natively — no conversion needed, no data loss.

**BQ and VBQ with 2-bit encoding** cannot represent N. The `-p` flag controls
what happens:

- `-p r` — replace each N with a random base (A/C/G/T) — **this is the default**
- `-p i` — skip the entire read if it contains any N
- `-p p` — error out (panic) if any N is encountered
- `-p a/c/g/t` — replace all Ns with that specific base

For most users on CBQ, this doesn't matter. But if you're using BQ or 2-bit VBQ
and your data has Ns, be aware of the default random replacement.

### How big are the files?

Rough expectations compared to gzipped FASTQ for typical Illumina short reads:

- **CBQ (default)**: comparable to or slightly smaller than `.fastq.gz`
- **CBQ with `-Q` (no quality)**: significantly smaller (quality scores are the
  bulk of FASTQ data)
- **CBQ with `-Q -H` (no quality, no headers)**: very compact, just sequences
- **BQ**: extremely compact (2 bits per base, no quality/headers), but lossy

The real advantage isn't just size — it's that bqtools can search and process
BINSEQ files directly without decompressing them first.

### Piping and stdout

bqtools commands are designed to work in Unix pipelines:

- **encode** reads from stdin if no input file is given
- **decode**, **grep**, and **sample** write to stdout by default (if no `-o`)
- You can chain commands, e.g., decode and pipe into another tool:

```bash
bqtools decode reads.cbq | head -n 400   # peek at first 100 reads (4 lines each)
bqtools grep reads.cbq "PATTERN" | wc -l  # count output lines
```

### Processing a slice of records (--span)

Most commands that read BINSEQ files accept `--span` to process only a range of
records by index. This is useful for quick spot-checks on huge files:

```bash
# Process only the first 10,000 records
bqtools decode reads.cbq --span ..10000 -o peek.fq

# Process records 50,000 through 60,000
bqtools grep reads.cbq "PATTERN" --span 50000..60000 -C

# From record 1,000,000 to end of file
bqtools sample reads.cbq --span 1000000.. -F 0.1 -o tail_sample.fq
```

## Installation

```bash
# Install via cargo (Rust package manager)
cargo install bqtools

# Verify it works
bqtools --help
```

If the user doesn't have Rust/cargo installed, they need to install it first:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

For fuzzy sequence matching support (optional, most users won't need this):

```bash
RUSTFLAGS="-C target-cpu=native" cargo install bqtools -F fuzzy
```

## Everyday Tasks

### Convert FASTQ → BINSEQ

This is the most common starting point. Take FASTQ files and store them as CBQ.

```bash
# Single file
bqtools encode reads.fastq -o reads.cbq

# Compressed input works automatically
bqtools encode reads.fastq.gz -o reads.cbq
bqtools encode reads.fastq.zst -o reads.cbq

# Paired-end reads (two files in, one CBQ out)
bqtools encode sample_R1.fastq.gz sample_R2.fastq.gz -o sample.cbq

# A whole directory of FASTQ files (each gets its own CBQ)
bqtools encode --recursive --mode cbq /path/to/fastqs/

# A whole directory of paired FASTQs
bqtools encode --recursive --paired --mode cbq /path/to/fastqs/

# Pipe from another tool
zcat reads.fastq.gz | bqtools encode -o reads.cbq
```

**Threads**: bqtools uses all available CPUs by default. Use `-T 4` to limit to 4.

**Smaller files**: Add `-Q` to skip quality scores or `-H` to skip headers if you
don't need them downstream.

### Convert BINSEQ → FASTQ

When you need FASTQ again (for a tool that doesn't support BINSEQ):

```bash
# Back to FASTQ
bqtools decode reads.cbq -o reads.fastq

# Compressed output (detected from extension)
bqtools decode reads.cbq -o reads.fastq.gz
bqtools decode reads.cbq -o reads.fastq.zst

# To FASTA instead
bqtools decode reads.cbq -o reads.fasta -f a

# To TSV (header + sequence, tab-separated)
bqtools decode reads.cbq -o reads.tsv -f t

# Paired-end → separate R1/R2 files
bqtools decode sample.cbq --prefix output/sample
# Creates: output/sample_R1.fq and output/sample_R2.fq

# Extract just R1 or just R2
bqtools decode sample.cbq -o r1_only.fq -m 1
bqtools decode sample.cbq -o r2_only.fq -m 2
```

### Search for Sequences (grep)

Search for DNA patterns directly in BINSEQ files — no decoding needed.

**R1/R2 terminology**: In paired BINSEQ files, R1 is called "primary" and R2 is
called "extended". This matters for flags:

- CLI patterns: `-r` = R1 only, `-R` = R2 only, bare positional = either
- Pattern files: `--sfile` = R1 only, `--xfile` = R2 only, `--file` = either

```bash
# Find reads containing a sequence
bqtools grep reads.cbq "AGATCGGAAGAGC"

# Just count how many reads match
bqtools grep reads.cbq "AGATCGGAAGAGC" -C

# Count with fraction of total
bqtools grep reads.cbq "AGATCGGAAGAGC" -F

# Save matching reads to a file
bqtools grep reads.cbq "AGATCGGAAGAGC" -o matches.fq

# Search with a regex
bqtools grep reads.cbq "ACGT[AC]{3,5}TCCA"

# Search only in R1 or R2 of paired data
bqtools grep reads.cbq -r "PATTERN_IN_R1"
bqtools grep reads.cbq -R "PATTERN_IN_R2"

# Multiple patterns (all must match by default)
bqtools grep reads.cbq "PATTERN1" "PATTERN2"

# Multiple patterns, any can match
bqtools grep reads.cbq "PATTERN1" "PATTERN2" --or-logic

# Exclude matching reads (invert, like grep -v)
bqtools grep reads.cbq "CONTAMINANT" -v -o clean.fq

# Only search within a basepair range (e.g., first 30bp)
bqtools grep reads.cbq "BARCODE" --range ..30
```

**Searching many patterns at once** (e.g., a barcode list):

```bash
# Patterns from a file (one per line), searches either R1 or R2
bqtools grep reads.cbq --file barcodes.txt

# Patterns targeting R1 only (primary)
bqtools grep reads.cbq --sfile barcodes.txt

# Patterns targeting R2 only (extended)
bqtools grep reads.cbq --xfile barcodes.txt

# Use -x for fixed strings — much faster with many patterns
bqtools grep reads.cbq --file barcodes.txt -x

# Count how many reads each pattern matches
bqtools grep reads.cbq --file barcodes.txt -Px
```

Pattern files use **OR logic** (any pattern can match). You can combine file
patterns with CLI patterns too.

The `-x` flag tells bqtools the patterns are literal strings (not regex), which
enables a faster algorithm. bqtools auto-detects this when all patterns are pure
A/C/G/T, but `-x` makes it explicit.

**Per-pattern counting (`-P`)** outputs a TSV with three columns:

```
pattern    count    frac_total
ACGTACGT   1523     0.0152
TTTTAAAA   987      0.0099
GGGGCCCC   0        0.0000
```

A pattern is counted at most once per read (even if it appears multiple times in
that read). A single read can contribute to multiple pattern counts.

**Fuzzy matching** (if installed with `-F fuzzy`):

```bash
# Allow 1 edit distance (default)
bqtools grep reads.cbq "ACGTACGTACGT" -z

# Allow 2 edits
bqtools grep reads.cbq "ACGTACGTACGT" -z -k2

# Only inexact matches (skip exact hits)
bqtools grep reads.cbq "ACGTACGTACGT" -zi
```

### Combine Files

```bash
# Concatenate CBQ files (must all be the same format)
bqtools cat file1.cbq file2.cbq file3.cbq -o combined.cbq
```

### Subsample Reads

```bash
# Random 10% sample
bqtools sample reads.cbq -F 0.1 -o subset.fq

# 1% sample with a fixed seed (reproducible)
bqtools sample reads.cbq -F 0.01 -S 42 -o subset.fq
```

### Inspect a File

`bqtools info` tells you everything about a BINSEQ file without processing it.

```bash
# Full summary
bqtools info reads.cbq
```

For a CBQ file this prints something like:

```
-------------------------------
             File
-------------------------------
Format              : CBQ
Version             : 1
-------------------------------
           Metadata
-------------------------------
Paired              : true
Quality:            : true
Headers:            : true
Flags               : false
-------------------------------
          Compression
-------------------------------
Compression Level   : 3
Virtual Block Size  : 128.00 KB
Mean Block Size     : 94.32 KB
-------------------------------
            Data
-------------------------------
Number of blocks    : 412
Number of records   : 1500000
```

**Answering specific questions:**

```bash
# How many records (reads) are in this file?
bqtools info reads.cbq -n
# → prints just a number, e.g.: 1500000

# Is this file paired?
# Look for "Paired" in the info output:
bqtools info reads.cbq | grep Paired
# → "Paired              : true" or "Paired              : false"

# How many blocks?
bqtools info reads.cbq | grep "Number of blocks"

# Does this file have quality scores?
bqtools info reads.cbq | grep Quality

# Does this file have headers?
bqtools info reads.cbq | grep Headers
```

**For scripting**, `-n` is the most useful — it gives you just the record count
as a plain number so you can capture it in a variable:

```bash
num_reads=$(bqtools info reads.cbq -n)
echo "File contains $num_reads reads"
```

**Advanced inspection:**

```bash
# Show CBQ block headers (low-level, for debugging)
bqtools info reads.cbq --show-headers

# Show VBQ index (legacy format)
bqtools info reads.vbq --show-index
```

### Stream to Legacy Tools (Named Pipes)

When a tool only accepts FASTQ files, you can stream BINSEQ data through named pipes
without writing intermediate files to disk:

```bash
# Create 4 pipes and stream in the background
bqtools pipe reads.cbq -p 4 -b /tmp/fifo &

# Process each pipe with your tool
ls /tmp/fifo_*.fq | xargs -P 4 -I {} sh -c 'your_tool {} > {}.out'
```

For paired-end data, this automatically creates `_R1`/`_R2` pipe pairs.

## Recipe: Common Automation Patterns

### Encode an entire sequencing run

```bash
# Convert all paired FASTQs in a directory tree to CBQ
bqtools encode --recursive --paired --mode cbq /data/sequencing_run/
```

### Check for adapter contamination

```bash
# Count reads with Illumina adapter
bqtools grep reads.cbq "AGATCGGAAGAGC" -x -F
```

### Extract reads matching a barcode whitelist

```bash
# barcodes.txt has one barcode per line
# Search in first 30bp of R1, save matching reads
bqtools grep reads.cbq --sfile barcodes.txt -x --range ..30 -o matched.fq
```

### Quick QC subsample

```bash
# Grab 0.1% of reads for a quick look
bqtools sample large_run.cbq -F 0.001 -o qc.fq
```

### Round-trip: encode, process, decode

```bash
# Encode
bqtools encode sample_R1.fq.gz sample_R2.fq.gz -o sample.cbq

# Search / filter / sample as needed
bqtools grep sample.cbq "INTERESTING_MOTIF" -o hits.fq

# Decode back to paired FASTQs when done
bqtools decode sample.cbq --prefix results/sample
```

### Check protospacer representation in Perturb-seq data

You have 10X Perturb-seq FASTQs and a guide library TSV. You want to verify all
protospacers are present without aligning. Protospacers are expected in R2.

```bash
# 1. Extract protospacer sequences from your library TSV (adjust column number)
awk -F'\t' 'NR>1 {print $2}' guide_library.tsv > protospacers.txt

# 2. Encode the paired FASTQs
bqtools encode sample_R1.fastq.gz sample_R2.fastq.gz -o sample.cbq

# 3. Count each protospacer in R2 (--xfile = R2/extended only)
bqtools grep sample.cbq --xfile protospacers.txt -Px > spacer_counts.tsv

# 4. Check for missing guides (count = 0)
awk -F'\t' 'NR>1 && $2==0' spacer_counts.tsv
```

The output TSV has columns: `pattern`, `count`, `frac_total`. Any row with count 0
is a missing guide. You can also look at the distribution of counts to assess
uniformity.

If you're unsure whether guides are in R1 or R2, use `--file` instead of `--xfile`
to search both.

### Check pooled library diversity after cloning

You've cloned a designed oligo library (sgRNAs, barcodes, variant sequences) into
plasmids and sequenced the pool. You want to know what fraction of your library is
represented and how uniform the coverage is.

```bash
# 1. Get your reference sequences into a text file (one per line)
cut -f1 library_oligos.tsv | tail -n+2 > expected_sequences.txt

# 2. Encode the sequencing data
bqtools encode pool_R1.fastq.gz pool_R2.fastq.gz -o pool.cbq

# 3. Count each oligo across both reads
bqtools grep pool.cbq --file expected_sequences.txt -Px > library_counts.tsv

# 4. Summary stats
total=$(wc -l < expected_sequences.txt)
detected=$(awk -F'\t' 'NR>1 && $2>0' library_counts.tsv | wc -l)
echo "$detected / $total library members detected"
```

### Screen for common contaminants

Quick check for mycoplasma, PhiX, or E. coli contamination using short marker
sequences — no alignment needed.

```bash
# contaminants.txt contains diagnostic sequences, one per line, e.g.:
# GCTCCTAAAAGGTTACTCCAG   (mycoplasma)
# GAGTTTTATCGCTTCCATGAC   (PhiX)
# AGTCGTATTGCACTCGTGATG   (E. coli)

bqtools grep reads.cbq --file contaminants.txt -Px
```

Any pattern with a non-trivial fraction suggests contamination worth investigating.

## Quick Flag Reference

| Flag             | Command              | What it does                                                      |
| ---------------- | -------------------- | ----------------------------------------------------------------- |
| `-o`             | all                  | Output file path                                                  |
| `-T N`           | all                  | Number of threads (0 = auto)                                      |
| `--span`         | decode, grep, sample | Process only a range of records (e.g., `..10000`, `50000..60000`) |
| `-f a/q/t`       | decode, grep         | Output format: fasta/fastq/tsv                                    |
| `-m 1/2/both`    | decode, grep         | Which mate to output                                              |
| `--prefix`       | decode               | Split paired output into R1/R2 files                              |
| `-Q`             | encode               | Skip quality scores                                               |
| `-H`             | encode               | Skip sequence headers                                             |
| `-A`             | encode               | Archive mode (4-bit, keep everything)                             |
| `-C`             | grep                 | Count matches                                                     |
| `-F`             | grep                 | Count + show fraction                                             |
| `-P`             | grep                 | Per-pattern count                                                 |
| `-v`             | grep                 | Invert match                                                      |
| `-x`             | grep                 | Fixed-string mode (faster)                                        |
| `-r`             | grep                 | Pattern in R1 only                                                |
| `-R`             | grep                 | Pattern in R2 only                                                |
| `--range`        | grep                 | Restrict search to basepair range                                 |
| `--or-logic`     | grep                 | Any pattern matches (default: all must)                           |
| `--file`         | grep                 | Load patterns from file (search either R1 or R2)                  |
| `--sfile`        | grep                 | Load patterns from file (R1/primary only)                         |
| `--xfile`        | grep                 | Load patterns from file (R2/extended only)                        |
| `-z`             | grep                 | Fuzzy matching                                                    |
| `-k N`           | grep                 | Max edit distance for fuzzy                                       |
| `-F 0.N`         | sample               | Fraction to sample                                                |
| `-S N`           | sample               | Random seed                                                       |
| `-n`             | info                 | Print only record count                                           |
| `--show-headers` | info                 | Print CBQ block headers                                           |
| `--show-index`   | info                 | Print VBQ index                                                   |
| `-p N`           | pipe                 | Number of named pipes                                             |
| `-b`             | pipe                 | Base path for pipe names                                          |
