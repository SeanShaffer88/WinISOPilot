//! A single local diagnostics file for support: version, host facts,
//! settings, the plan and log tails, with the user's name and profile path
//! replaced. Nothing is uploaded; the user decides where to save it.

/// Replaces each `(needle, placeholder)` case-insensitively (ASCII).
pub fn redact(text: &str, pairs: &[(String, &str)]) -> String {
    let mut out = text.to_string();
    for (needle, placeholder) in pairs {
        if needle.len() < 3 {
            continue;
        }
        let lower_needle = needle.to_ascii_lowercase();
        let mut result = String::with_capacity(out.len());
        let lower = out.to_ascii_lowercase();
        let mut i = 0;
        while let Some(pos) = lower[i..].find(&lower_needle) {
            result.push_str(&out[i..i + pos]);
            result.push_str(placeholder);
            i += pos + needle.len();
        }
        result.push_str(&out[i..]);
        out = result;
    }
    out
}

/// Last `max` bytes of `text`, starting at a line boundary.
pub fn tail(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut start = text.len() - max;
    while !text.is_char_boundary(start) {
        start += 1;
    }
    match text[start..].find('\n') {
        Some(n) => &text[start + n + 1..],
        None => &text[start..],
    }
}

/// Assembles the file from titled sections.
pub fn bundle(sections: &[(String, String)]) -> String {
    let mut s = format!("WinISOPilot {} diagnostics\n", crate::APP_VERSION);
    s += "Personal paths and the account name are replaced. Review before sharing.\n";
    for (title, body) in sections {
        s += &format!("\n===== {title} =====\n{}\n", body.trim_end());
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_case_insensitively() {
        let pairs = vec![(r"C:\Users\Sean".to_string(), "%USERPROFILE%"), ("Sean".to_string(), "<user>")];
        let t = redact(r"Saved c:\users\sean\Desktop\x.iso by SEAN", &pairs);
        assert_eq!(t, r"Saved %USERPROFILE%\Desktop\x.iso by <user>");
        assert_eq!(redact("ab", &[("a".to_string(), "x")]), "ab");
    }

    #[test]
    fn tails_on_line_boundary() {
        let t = "one\ntwo\nthree\n";
        assert_eq!(tail(t, 100), t);
        assert_eq!(tail(t, 8), "three\n");
        assert!(bundle(&[("A".into(), "b".into())]).contains("===== A ====="));
    }
}
