use std::{fmt::Display, str::FromStr};

#[derive(Clone, Copy, Debug)]
pub struct SimpleRange {
    start: Option<usize>,
    end: Option<usize>,
}
impl SimpleRange {
    pub fn validate(&self) -> Result<(), &'static str> {
        if let Some(start) = self.start {
            if let Some(end) = self.end {
                if start > end {
                    return Err("Start must be less than or equal to end");
                }
            }
        }
        Ok(())
    }
    pub fn slice<'a>(&self, buf: &'a [u8]) -> &'a [u8] {
        match (self.start, self.end) {
            (None, None) => buf,
            (None, Some(end)) => &buf[..end.min(buf.len())],
            (Some(start), None) => &buf[start.min(buf.len())..],
            (Some(start), Some(end)) => &buf[start.min(buf.len())..end.min(buf.len())],
        }
    }
    pub fn offset(&self) -> usize {
        self.start.unwrap_or(0)
    }
}
impl Display for SimpleRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (self.start, self.end) {
            (None, None) => write!(f, ".."),
            (None, Some(end)) => write!(f, "..{end}"),
            (Some(start), None) => write!(f, "{start}.."),
            (Some(start), Some(end)) => write!(f, "{start}..{end}"),
        }
    }
}
impl FromStr for SimpleRange {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let range = if s.is_empty() || s == ".." {
            Self {
                start: None,
                end: None,
            }
        } else if s.starts_with("..") {
            let end = s[2..].parse().map_err(|_| "Invalid range")?;
            Self {
                start: None,
                end: Some(end),
            }
        } else if s.ends_with("..") {
            let start = s[..s.len() - 2].parse().map_err(|_| "Invalid range")?;
            Self {
                start: Some(start),
                end: None,
            }
        } else {
            let (start_str, end_str) = s.split_once("..").ok_or("Invalid range")?;
            let start = start_str.parse().map_err(|_| "Invalid range")?;
            let end = end_str.parse().map_err(|_| "Invalid range")?;
            Self {
                start: Some(start),
                end: Some(end),
            }
        };
        range.validate()?;
        Ok(range)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_valid_ranges() {
        // None, None should be valid
        let range = SimpleRange {
            start: None,
            end: None,
        };
        assert!(range.validate().is_ok());

        // None, Some should be valid
        let range = SimpleRange {
            start: None,
            end: Some(10),
        };
        assert!(range.validate().is_ok());

        // Some, None should be valid
        let range = SimpleRange {
            start: Some(5),
            end: None,
        };
        assert!(range.validate().is_ok());

        // Some, Some with start <= end should be valid
        let range = SimpleRange {
            start: Some(5),
            end: Some(10),
        };
        assert!(range.validate().is_ok());

        // Equal start and end should be valid
        let range = SimpleRange {
            start: Some(5),
            end: Some(5),
        };
        assert!(range.validate().is_ok());
    }

    #[test]
    fn test_validate_invalid_ranges() {
        // start > end should be invalid
        let range = SimpleRange {
            start: Some(10),
            end: Some(5),
        };
        assert_eq!(
            range.validate(),
            Err("Start must be less than or equal to end")
        );
    }

    #[test]
    fn test_slice_full_range() {
        let range = SimpleRange {
            start: None,
            end: None,
        };
        let buf = b"hello world";
        assert_eq!(range.slice(buf), b"hello world");
    }

    #[test]
    fn test_slice_end_only() {
        let range = SimpleRange {
            start: None,
            end: Some(5),
        };
        let buf = b"hello world";
        assert_eq!(range.slice(buf), b"hello");

        // Test end beyond buffer length
        let range = SimpleRange {
            start: None,
            end: Some(20),
        };
        assert_eq!(range.slice(buf), b"hello world");
    }

    #[test]
    fn test_slice_start_only() {
        let range = SimpleRange {
            start: Some(6),
            end: None,
        };
        let buf = b"hello world";
        assert_eq!(range.slice(buf), b"world");

        // Test start beyond buffer length
        let range = SimpleRange {
            start: Some(20),
            end: None,
        };
        assert_eq!(range.slice(buf), b"");
    }

    #[test]
    fn test_slice_start_and_end() {
        let range = SimpleRange {
            start: Some(3),
            end: Some(8),
        };
        let buf = b"hello world";
        assert_eq!(range.slice(buf), b"lo wo");

        // Test both beyond buffer length
        let range = SimpleRange {
            start: Some(20),
            end: Some(25),
        };
        assert_eq!(range.slice(buf), b"");

        // Test start at buffer length
        let range = SimpleRange {
            start: Some(11),
            end: Some(15),
        };
        assert_eq!(range.slice(buf), b"");
    }

    #[test]
    fn test_slice_edge_cases() {
        let range = SimpleRange {
            start: Some(0),
            end: Some(0),
        };
        let buf = b"hello";
        assert_eq!(range.slice(buf), b"");

        let range = SimpleRange {
            start: Some(0),
            end: Some(5),
        };
        assert_eq!(range.slice(buf), b"hello");

        // Empty buffer
        let buf = b"";
        let range = SimpleRange {
            start: Some(0),
            end: Some(5),
        };
        assert_eq!(range.slice(buf), b"");
    }

    #[test]
    fn test_offset() {
        let range = SimpleRange {
            start: None,
            end: None,
        };
        assert_eq!(range.offset(), 0);

        let range = SimpleRange {
            start: None,
            end: Some(10),
        };
        assert_eq!(range.offset(), 0);

        let range = SimpleRange {
            start: Some(5),
            end: None,
        };
        assert_eq!(range.offset(), 5);

        let range = SimpleRange {
            start: Some(7),
            end: Some(10),
        };
        assert_eq!(range.offset(), 7);
    }

    #[test]
    fn test_display() {
        let range = SimpleRange {
            start: None,
            end: None,
        };
        assert_eq!(format!("{}", range), "..");

        let range = SimpleRange {
            start: None,
            end: Some(10),
        };
        assert_eq!(format!("{}", range), "..10");

        let range = SimpleRange {
            start: Some(5),
            end: None,
        };
        assert_eq!(format!("{}", range), "5..");

        let range = SimpleRange {
            start: Some(3),
            end: Some(8),
        };
        assert_eq!(format!("{}", range), "3..8");
    }

    #[test]
    fn test_from_str_valid() {
        // Empty and ".." should parse to full range
        let range: SimpleRange = "".parse().unwrap();
        assert_eq!(range.start, None);
        assert_eq!(range.end, None);

        let range: SimpleRange = "..".parse().unwrap();
        assert_eq!(range.start, None);
        assert_eq!(range.end, None);

        // End only
        let range: SimpleRange = "..10".parse().unwrap();
        assert_eq!(range.start, None);
        assert_eq!(range.end, Some(10));

        // Start only
        let range: SimpleRange = "5..".parse().unwrap();
        assert_eq!(range.start, Some(5));
        assert_eq!(range.end, None);

        // Start and end
        let range: SimpleRange = "3..8".parse().unwrap();
        assert_eq!(range.start, Some(3));
        assert_eq!(range.end, Some(8));

        // Equal start and end
        let range: SimpleRange = "5..5".parse().unwrap();
        assert_eq!(range.start, Some(5));
        assert_eq!(range.end, Some(5));
    }

    #[test]
    fn test_from_str_invalid() {
        // Invalid number format
        assert!("..abc".parse::<SimpleRange>().is_err());
        assert!("abc..".parse::<SimpleRange>().is_err());
        assert!("abc..def".parse::<SimpleRange>().is_err());

        // Missing ".." separator
        assert!("123".parse::<SimpleRange>().is_err());

        // Start > end (validation should fail)
        assert!("10..5".parse::<SimpleRange>().is_err());

        // Invalid format with multiple ".."
        assert!("1..2..3".parse::<SimpleRange>().is_err());
    }

    #[test]
    fn test_roundtrip_display_parse() {
        let ranges = vec![
            SimpleRange {
                start: None,
                end: None,
            },
            SimpleRange {
                start: None,
                end: Some(10),
            },
            SimpleRange {
                start: Some(5),
                end: None,
            },
            SimpleRange {
                start: Some(3),
                end: Some(8),
            },
        ];

        for original in ranges {
            let string_repr = format!("{}", original);
            let parsed: SimpleRange = string_repr.parse().unwrap();
            assert_eq!(original.start, parsed.start);
            assert_eq!(original.end, parsed.end);
        }
    }
}
