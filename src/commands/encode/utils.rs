use std::{collections::HashMap, io::Read, path::PathBuf};

use anyhow::{bail, Result};
use log::warn;
use paraseq::{
    fastx,
    rust_htslib::{self, bam::Read as BamRead},
    Record,
};
use regex::Regex;

type BoxReader = Box<dyn Read + Send>;

pub fn get_sequence_len(reader: &mut fastx::Reader<BoxReader>) -> Result<u32> {
    let mut rset = reader.new_record_set_with_size(1);
    let slen = if rset.fill(reader)? {
        let record = if let Some(record) = rset.iter().next() {
            record?
        } else {
            bail!("Input file is empty - cannot convert");
        };
        record.seq().len()
    } else {
        bail!("Input file is empty - cannot convert");
    };
    reader.reload(&mut rset)?;
    Ok(slen as u32)
}

pub fn get_sequence_len_htslib(path: &str, paired: bool) -> Result<(u32, u32)> {
    let mut reader = rust_htslib::bam::Reader::from_path(path)?;
    let mut slen = 0;
    let mut xlen = 0;

    let mut rc_records = reader.rc_records();

    if let Some(res) = rc_records.next() {
        let rec = res?;
        slen = rec.seq_len();
    }

    if paired {
        if let Some(res) = rc_records.next() {
            let rec = res?;
            xlen = rec.seq_len();
        }
    }
    Ok((slen as u32, xlen as u32))
}

pub fn get_interleaved_sequence_len(reader: &mut fastx::Reader<BoxReader>) -> Result<(u32, u32)> {
    let mut rset = reader.new_record_set_with_size(2);
    let (slen, xlen) = if rset.fill(reader)? {
        let mut rset_iter = rset.iter();
        let r1 = if let Some(record) = rset_iter.next() {
            record?
        } else {
            bail!("Input file is empty - cannot convert");
        };
        let r2 = if let Some(record) = rset_iter.next() {
            record?
        } else {
            bail!("Input file is empty - cannot convert");
        };
        (r1.seq().len(), r2.seq().len())
    } else {
        bail!("Input file (interleaved) is missing R2 - cannot convert");
    };
    reader.reload(&mut rset)?;
    Ok((slen as u32, xlen as u32))
}

/// Pairs R1/R2 files from a list of file paths efficiently using a `HashMap`
/// Returns a vector of pairs, where each pair is [`R1_file`, `R2_file`]
pub fn pair_r1_r2_files(files: &[PathBuf]) -> Result<Vec<Vec<PathBuf>>> {
    let pair_regex = Regex::new(r"^(.+)_R([12])(_[^.]*)?\.(?:fastq|fq|fasta|fa)(?:\.gz|\.zst)?$")?;

    // HashMap to store files by their pairing key (base + suffix)
    let mut r1_files: HashMap<String, PathBuf> = HashMap::new();
    let mut r2_files: HashMap<String, PathBuf> = HashMap::new();

    // Single pass through files to categorize them
    for file in files {
        let file_str = file.to_str().unwrap();

        if let Some(caps) = pair_regex.captures(file_str) {
            let base = &caps[1];
            let read_num = &caps[2];
            let suffix = caps.get(3).map_or("", |m| m.as_str());

            // Create a unique key for pairing: base + suffix
            let pair_key = format!("{base}{suffix}");

            match read_num {
                "1" => {
                    r1_files.insert(pair_key, file.clone());
                }
                "2" => {
                    r2_files.insert(pair_key, file.clone());
                }
                _ => unreachable!(), // regex only matches 1 or 2
            }
        }
    }

    // Create pairs by finding matching keys
    let mut pairs = Vec::new();
    for (pair_key, r1_file) in &r1_files {
        if let Some(r2_file) = r2_files.get(pair_key) {
            pairs.push(vec![r1_file.to_owned(), r2_file.to_owned()]);
        } else {
            warn!("No R2 pair found for {}", r1_file.display());
        }
    }

    // Check for orphaned R2 files
    for (pair_key, r2_file) in &r2_files {
        if !r1_files.contains_key(pair_key) {
            warn!("No R1 pair found for {}", r2_file.display());
        }
    }

    // Sort pairs by R1 filename for consistent output
    pairs.sort_by(|a, b| a[0].cmp(&b[0]));

    Ok(pairs)
}

/// Generates a unique output filename based on the input file(s)
/// For single files: removes the original extension and replaces with new extension
/// For paired files: extracts the base name + suffix (everything except _R[12]) and adds new extension
pub fn generate_output_name(input_files: &[PathBuf], new_extension: &str) -> Result<String> {
    match input_files.len() {
        1 => {
            // Single file: just replace the extension
            let input_path = input_files[0].to_str().unwrap();
            let extension_regex = Regex::new(r"\.(?:fastq|fq|fasta|fa)(?:\.gz|\.zst)?$")?;
            let output_name = extension_regex
                .replace(input_path, new_extension)
                .to_string();
            Ok(output_name)
        }
        2 => {
            // Paired files: extract base name + suffix, excluding _R[12]
            let input_path = input_files[0].to_str().unwrap();
            let pair_regex =
                Regex::new(r"^(.+)_R[12](_[^.]*)?\.(?:fastq|fq|fasta|fa)(?:\.gz|\.zst)?$")?;

            if let Some(caps) = pair_regex.captures(input_path) {
                let base = &caps[1];
                let suffix = caps.get(2).map_or("", |m| m.as_str());
                let output_name = format!("{base}{suffix}{new_extension}");
                Ok(output_name)
            } else {
                // Fallback: use the first file's name with extension replaced
                let extension_regex = Regex::new(r"\.(?:fastq|fq|fasta|fa)(?:\.gz|\.zst)?$")?;
                let output_name = extension_regex
                    .replace(input_path, new_extension)
                    .to_string();
                Ok(output_name)
            }
        }
        _ => bail!("Invalid number of input files: {}", input_files.len()),
    }
}

pub fn pull_single_files(input_files: &[PathBuf]) -> Result<Vec<Vec<PathBuf>>> {
    let mut num_suspect = 0;
    let pair_regex = Regex::new(r".+_R[12].+")?;
    let mut pqueue = Vec::new();
    for file in input_files {
        let file_str = file.to_str().unwrap();
        if pair_regex.is_match(file_str) {
            num_suspect += 1;
        }
        pqueue.push(vec![file.to_owned()])
    }
    if num_suspect > 0 {
        warn!(
            "Found {} files that may be paired but are not. If this is not intentional, consider adding the `--paired` flag.",
            num_suspect
        );
    }
    Ok(pqueue)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_pair_r1_r2_files_sequential() {
        let files = vec![
            PathBuf::from("sample_0_R1_001.fq"),
            PathBuf::from("sample_0_R2_001.fq"),
            PathBuf::from("sample_1_R1.fastq"),
            PathBuf::from("sample_1_R2.fastq"),
        ];

        let pairs = pair_r1_r2_files(&files).unwrap();
        assert_eq!(pairs.len(), 2);

        assert_eq!(pairs[0][0], PathBuf::from("sample_0_R1_001.fq"));
        assert_eq!(pairs[0][1], PathBuf::from("sample_0_R2_001.fq"));

        assert_eq!(pairs[1][0], PathBuf::from("sample_1_R1.fastq"));
        assert_eq!(pairs[1][1], PathBuf::from("sample_1_R2.fastq"));
    }

    #[test]
    fn test_pair_r1_r2_files_non_sequential() {
        // This is the key test case you mentioned
        let files = vec![
            PathBuf::from("library_A_R1_lane1.fastq"),
            PathBuf::from("library_A_R1_lane2.fastq"),
            PathBuf::from("library_A_R2_lane1.fastq"),
            PathBuf::from("library_A_R2_lane2.fastq"),
        ];

        let pairs = pair_r1_r2_files(&files).unwrap();
        assert_eq!(pairs.len(), 2);

        // Should pair lane1 with lane1, lane2 with lane2
        assert_eq!(pairs[0][0], PathBuf::from("library_A_R1_lane1.fastq"));
        assert_eq!(pairs[0][1], PathBuf::from("library_A_R2_lane1.fastq"));

        assert_eq!(pairs[1][0], PathBuf::from("library_A_R1_lane2.fastq"));
        assert_eq!(pairs[1][1], PathBuf::from("library_A_R2_lane2.fastq"));
    }

    #[test]
    fn test_pair_r1_r2_files_missing_pairs() {
        let files = vec![
            PathBuf::from("sample_1_R1.fastq"),
            PathBuf::from("sample_2_R2.fastq"), // Missing R1
            PathBuf::from("sample_3_R1.fastq"), // Missing R2
        ];

        let pairs = pair_r1_r2_files(&files).unwrap();
        assert_eq!(pairs.len(), 0); // No complete pairs
    }

    #[test]
    fn test_large_file_list_performance() {
        // Generate a large list to ensure O(n) performance
        let mut files = Vec::new();
        for i in 0..10000 {
            files.push(PathBuf::from(format!("sample_{:04}_R1_lane1.fastq", i)));
            files.push(PathBuf::from(format!("sample_{:04}_R2_lane1.fastq", i)));
        }

        let pairs = pair_r1_r2_files(&files).unwrap();
        assert_eq!(pairs.len(), 10000);
    }

    #[test]
    fn test_generate_output_name_single_file() {
        let files = vec![PathBuf::from("sample_001.fastq")];
        let output = generate_output_name(&files, ".encoded").unwrap();
        assert_eq!(output, "sample_001.encoded");
    }

    #[test]
    fn test_generate_output_name_single_file_compressed() {
        let files = vec![PathBuf::from("sample_001.fastq.gz")];
        let output = generate_output_name(&files, ".encoded").unwrap();
        assert_eq!(output, "sample_001.encoded");
    }

    #[test]
    fn test_generate_output_name_paired_files() {
        let files = vec![
            PathBuf::from("sample_001_R1.fastq"),
            PathBuf::from("sample_001_R2.fastq"),
        ];
        let output = generate_output_name(&files, ".encoded").unwrap();
        assert_eq!(output, "sample_001.encoded");
    }

    #[test]
    fn test_generate_output_name_paired_files_with_suffix() {
        let files = vec![
            PathBuf::from("library_A_R1_lane1.fastq"),
            PathBuf::from("library_A_R2_lane1.fastq"),
        ];
        let output = generate_output_name(&files, ".encoded").unwrap();
        assert_eq!(output, "library_A_lane1.encoded");
    }

    #[test]
    fn test_generate_output_name_paired_files_complex_suffix() {
        let files = vec![
            PathBuf::from("sample_0_R1_001.fq.gz"),
            PathBuf::from("sample_0_R2_001.fq.gz"),
        ];
        let output = generate_output_name(&files, ".encoded").unwrap();
        assert_eq!(output, "sample_0_001.encoded");
    }

    #[test]
    fn test_generate_output_name_different_lane_numbers() {
        // This test shows that different lanes get different output names
        let files1 = vec![
            PathBuf::from("library_A_R1_lane1.fastq"),
            PathBuf::from("library_A_R2_lane1.fastq"),
        ];
        let files2 = vec![
            PathBuf::from("library_A_R1_lane2.fastq"),
            PathBuf::from("library_A_R2_lane2.fastq"),
        ];

        let output1 = generate_output_name(&files1, ".encoded").unwrap();
        let output2 = generate_output_name(&files2, ".encoded").unwrap();

        assert_eq!(output1, "library_A_lane1.encoded");
        assert_eq!(output2, "library_A_lane2.encoded");
        assert_ne!(output1, output2); // Ensure they're different
    }
}
