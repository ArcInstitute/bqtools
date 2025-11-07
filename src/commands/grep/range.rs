use std::{fmt::Display, str::FromStr};

#[derive(Clone, Copy, Debug)]
pub struct SimpleRange {
    pub start: Option<usize>,
    pub end: Option<usize>,
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
            let parts = s.split("..").collect::<Vec<_>>();
            if parts.len() != 2 {
                return Err("Invalid range");
            }
            let start = parts[0].parse().map_err(|_| "Invalid range")?;
            let end = parts[1].parse().map_err(|_| "Invalid range")?;
            Self {
                start: Some(start),
                end: Some(end),
            }
        };
        range.validate()?;
        Ok(range)
    }
}
