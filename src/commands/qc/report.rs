/// Renders a markdown table. Returns an empty string if `rows` is empty, so
/// callers can unconditionally splice the result into a report without an
/// extra emptiness check.
pub fn table(headers: &[&str], rows: &[Vec<String>]) -> String {
    use std::fmt::Write as _;

    if rows.is_empty() {
        return String::new();
    }

    let mut out = format!("| {} |\n", headers.join(" | "));
    let _ = writeln!(
        out,
        "|{}|",
        headers.iter().map(|_| "---").collect::<Vec<_>>().join("|")
    );
    for row in rows {
        let _ = writeln!(out, "| {} |", row.join(" | "));
    }
    out
}

/// Wraps a module's already-rendered primary/extended (R1/R2) summary
/// bodies under one heading. `None` means that side had no data to report
/// (e.g. `extended` is always `None` for single-end input). Returns an empty
/// string if both sides are `None`, so callers can splice the result in
/// unconditionally.
pub fn dual_section(title: &str, primary: Option<String>, extended: Option<String>) -> String {
    if primary.is_none() && extended.is_none() {
        return String::new();
    }

    let split = primary.is_some() && extended.is_some();
    let mut out = format!("## {title}\n\n");
    if let Some(body) = primary {
        if split {
            out.push_str("### R1\n\n");
        }
        out.push_str(&body);
        out.push('\n');
    }
    if let Some(body) = extended {
        if split {
            out.push_str("### R2\n\n");
        }
        out.push_str(&body);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_with_no_rows_is_empty() {
        assert_eq!(table(&["A", "B"], &[]), "");
    }

    #[test]
    fn table_renders_header_separator_and_rows() {
        let rows = vec![
            vec!["1".to_string(), "2".to_string()],
            vec!["3".to_string(), "4".to_string()],
        ];
        assert_eq!(
            table(&["A", "B"], &rows),
            "| A | B |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |\n"
        );
    }

    #[test]
    fn dual_section_with_neither_side_is_empty() {
        assert_eq!(dual_section("Title", None, None), "");
    }

    #[test]
    fn dual_section_primary_only_has_no_r1_r2_headings() {
        let out = dual_section("Title", Some("body\n".to_string()), None);
        assert!(out.contains("## Title"));
        assert!(out.contains("body"));
        assert!(!out.contains("### R1"));
        assert!(!out.contains("### R2"));
    }

    #[test]
    fn dual_section_extended_only_has_no_r1_r2_headings() {
        let out = dual_section("Title", None, Some("body\n".to_string()));
        assert!(out.contains("## Title"));
        assert!(out.contains("body"));
        assert!(!out.contains("### R1"));
        assert!(!out.contains("### R2"));
    }

    #[test]
    fn dual_section_both_sides_split_r1_before_r2() {
        let out = dual_section(
            "Title",
            Some("primary body\n".to_string()),
            Some("extended body\n".to_string()),
        );
        assert!(out.contains("primary body"));
        assert!(out.contains("extended body"));
        assert!(out.find("### R1").unwrap() < out.find("### R2").unwrap());
    }
}
