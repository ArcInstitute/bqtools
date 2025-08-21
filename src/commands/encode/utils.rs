use std::{collections::HashMap, io::Read, path::PathBuf};

use anyhow::{bail, Result};
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

/// Pairs R1/R2 files from a list of file paths efficiently using a HashMap
/// Returns a vector of pairs, where each pair is [R1_file, R2_file]
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
            let pair_key = format!("{}{}", base, suffix);

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
            eprintln!("Warning: No R2 pair found for {}", r1_file.display());
        }
    }

    // Check for orphaned R2 files
    for (pair_key, r2_file) in &r2_files {
        if !r1_files.contains_key(pair_key) {
            eprintln!("Warning: No R1 pair found for {}", r2_file.display());
        }
    }

    // Sort pairs by R1 filename for consistent output
    pairs.sort_by(|a, b| a[0].cmp(&b[0]));

    Ok(pairs)
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
}
