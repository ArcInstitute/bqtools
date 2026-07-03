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
