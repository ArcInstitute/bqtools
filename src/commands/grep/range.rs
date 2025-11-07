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
            (None, None) => &buf[..],
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
            (None, Some(end)) => write!(f, "..{}", end),
            (Some(start), None) => write!(f, "{}..", start),
            (Some(start), Some(end)) => write!(f, "{}..{}", start, end),
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
