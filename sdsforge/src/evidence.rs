/// Source-text evidence extraction and JSON coverage matching.
///
/// Extracts chemical identifiers (CAS, H/P-codes, UN numbers) and signal words
/// from raw SDS text, then measures how well the generated JSON reflects them.
/// Used by `eval-corpus` to compute evidence_coverage without requiring gold labels.

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Default, serde::Serialize)]
pub struct Evidence {
    pub cas: Vec<String>,
    pub h_codes: Vec<String>,
    pub p_codes: Vec<String>,
    pub un_numbers: Vec<String>,
    pub signal_words: Vec<String>,
}

#[derive(Debug, Default, serde::Serialize)]
pub struct EvidenceCoverage {
    /// Fraction of source CAS numbers found anywhere in the JSON (0.0–1.0).
    pub cas: f32,
    /// Fraction of source H-codes found in the JSON.
    pub h_codes: f32,
    /// Fraction of source P-codes found in the JSON.
    pub p_codes: f32,
    /// Fraction of source UN numbers found in the JSON.
    pub un_numbers: f32,
}

// ---------------------------------------------------------------------------
// Extraction
// ---------------------------------------------------------------------------

/// Extract chemical identifiers and signal words from raw SDS text.
pub fn extract_evidence(text: &str) -> Evidence {
    Evidence {
        cas:          extract_cas(text),
        h_codes:      extract_h_codes(text),
        p_codes:      extract_p_codes(text),
        un_numbers:   extract_un_numbers(text),
        signal_words: extract_signal_words(text),
    }
}

/// CAS: digits-digits-digit (e.g. "67-56-1", "1336-21-6")
fn extract_cas(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i < len {
        // Find a digit
        if !bytes[i].is_ascii_digit() { i += 1; continue; }
        // Scan digits (2–7)
        let a_start = i;
        while i < len && bytes[i].is_ascii_digit() { i += 1; }
        let a_len = i - a_start;
        if a_len < 2 || a_len > 7 { continue; }
        // Expect '-'
        if i >= len || bytes[i] != b'-' { continue; }
        i += 1;
        // Expect 2 digits
        let b_start = i;
        while i < len && bytes[i].is_ascii_digit() { i += 1; }
        if i - b_start != 2 { continue; }
        // Expect '-'
        if i >= len || bytes[i] != b'-' { continue; }
        i += 1;
        // Expect 1 digit
        if i >= len || !bytes[i].is_ascii_digit() { continue; }
        i += 1;
        // Must not be followed by a digit (avoid matching in larger numbers)
        if i < len && bytes[i].is_ascii_digit() { continue; }
        // Must not be preceded by a digit at a_start-1
        if a_start > 0 && bytes[a_start - 1].is_ascii_digit() { continue; }
        let cas = &text[a_start..i];
        if !out.contains(&cas.to_string()) {
            out.push(cas.to_string());
        }
    }
    out
}

/// H-codes: H followed by 3 digits, optionally joined by '+' (e.g. H225, H301+H311)
fn extract_h_codes(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;
    while i < len {
        if chars[i] != 'H' && chars[i] != 'h' { i += 1; continue; }
        // Preceded by word char? Skip (e.g. "REACH" shouldn't match)
        if i > 0 && (chars[i-1].is_alphanumeric() || chars[i-1] == '_') { i += 1; continue; }
        let start = i;
        i += 1;
        // 3 digits
        let d_start = i;
        while i < len && chars[i].is_ascii_digit() { i += 1; }
        if i - d_start != 3 { continue; }
        // Check it's followed by a non-alphanumeric (or end)
        if i < len && chars[i].is_alphanumeric() { continue; }
        let code = chars[start..i].iter().collect::<String>().to_uppercase();
        if !out.contains(&code) {
            out.push(code);
        }
    }
    out
}

/// P-codes: P followed by 3 digits, optionally joined by '+' (e.g. P260, P301+P330)
fn extract_p_codes(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;
    while i < len {
        if chars[i] != 'P' && chars[i] != 'p' { i += 1; continue; }
        if i > 0 && (chars[i-1].is_alphanumeric() || chars[i-1] == '_') { i += 1; continue; }
        let start = i;
        i += 1;
        let d_start = i;
        while i < len && chars[i].is_ascii_digit() { i += 1; }
        if i - d_start != 3 { continue; }
        if i < len && chars[i].is_alphanumeric() { continue; }
        let code = chars[start..i].iter().collect::<String>().to_uppercase();
        if !out.contains(&code) {
            out.push(code);
        }
    }
    out
}

/// UN numbers: UN followed by optional whitespace and 4 digits (e.g. "UN 1230", "UN1090")
fn extract_un_numbers(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i < len {
        // Match "UN" (case-insensitive)
        if i + 1 < len
            && (bytes[i] == b'U' || bytes[i] == b'u')
            && (bytes[i+1] == b'N' || bytes[i+1] == b'n')
        {
            // Not preceded by alphanumeric
            if i > 0 && bytes[i-1].is_ascii_alphanumeric() { i += 1; continue; }
            let mut j = i + 2;
            // Optional whitespace / separator
            while j < len && (bytes[j] == b' ' || bytes[j] == b'\t' || bytes[j] == b':' || bytes[j] == b'.') {
                j += 1;
            }
            // Expect exactly 4 digits
            let d_start = j;
            while j < len && bytes[j].is_ascii_digit() { j += 1; }
            if j - d_start == 4 {
                // Not followed by digit
                if j < len && bytes[j].is_ascii_digit() { i += 1; continue; }
                let num = std::str::from_utf8(&bytes[d_start..j]).unwrap_or("").to_string();
                if !out.contains(&num) {
                    out.push(num);
                }
                i = j;
                continue;
            }
        }
        i += 1;
    }
    out
}

const SIGNAL_PATTERNS: &[&str] = &[
    "危険", "警告", "Danger", "danger", "DANGER",
    "Warning", "warning", "WARNING",
    "危险", "警告", // zh-cn/zh-tw (same kanji)
    "Not classified", "not classified",
    "Not applicable",
];

fn extract_signal_words(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for &pat in SIGNAL_PATTERNS {
        if text.contains(pat) && !out.iter().any(|s: &String| s.eq_ignore_ascii_case(pat)) {
            out.push(pat.to_string());
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Coverage matching
// ---------------------------------------------------------------------------

/// Compute what fraction of each evidence category appears somewhere in the JSON.
pub fn match_evidence(ev: &Evidence, json: &serde_json::Value) -> EvidenceCoverage {
    let json_str = json.to_string().to_uppercase();
    EvidenceCoverage {
        cas:       coverage_ratio(&ev.cas,        |s| json_str.contains(&s.to_uppercase())),
        h_codes:   coverage_ratio(&ev.h_codes,    |s| json_str.contains(s)),
        p_codes:   coverage_ratio(&ev.p_codes,    |s| json_str.contains(s)),
        un_numbers:coverage_ratio(&ev.un_numbers, |s| json_str.contains(s)),
    }
}

fn coverage_ratio<F: Fn(&str) -> bool>(items: &[String], found: F) -> f32 {
    if items.is_empty() { return 1.0; } // nothing to check = perfect
    let matched = items.iter().filter(|s| found(s)).count();
    matched as f32 / items.len() as f32
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cas_basic() {
        let ev = extract_evidence("成分: Methanol CAS 67-56-1, Ethanol 64-17-5.");
        assert!(ev.cas.contains(&"67-56-1".to_string()), "got: {:?}", ev.cas);
        assert!(ev.cas.contains(&"64-17-5".to_string()), "got: {:?}", ev.cas);
    }

    #[test]
    fn h_p_codes() {
        let ev = extract_evidence("H225, H301+H311\nP260, P301+P330+P331");
        assert!(ev.h_codes.contains(&"H225".to_string()));
        assert!(ev.h_codes.contains(&"H301".to_string()));
        assert!(ev.p_codes.contains(&"P260".to_string()));
        assert!(ev.p_codes.contains(&"P301".to_string()));
    }

    #[test]
    fn un_number() {
        let ev = extract_evidence("UN 1230  UN1090 UN: 3077");
        assert!(ev.un_numbers.contains(&"1230".to_string()), "{:?}", ev.un_numbers);
        assert!(ev.un_numbers.contains(&"1090".to_string()), "{:?}", ev.un_numbers);
        assert!(ev.un_numbers.contains(&"3077".to_string()), "{:?}", ev.un_numbers);
    }

    #[test]
    fn signal_words() {
        let ev = extract_evidence("Signal word: Danger.\nWARNING for storage.");
        assert!(ev.signal_words.iter().any(|s| s.to_uppercase() == "DANGER"));
    }

    #[test]
    fn coverage_empty_source_is_perfect() {
        let ev = Evidence::default();
        let cov = match_evidence(&ev, &serde_json::Value::Null);
        assert_eq!(cov.cas, 1.0);
        assert_eq!(cov.h_codes, 1.0);
    }

    #[test]
    fn coverage_partial() {
        let ev = Evidence {
            cas: vec!["67-56-1".into(), "99-99-9".into()],
            ..Default::default()
        };
        let json = serde_json::json!({"CASno": "67-56-1"});
        let cov = match_evidence(&ev, &json);
        assert!((cov.cas - 0.5).abs() < 0.01, "expected 0.5, got {}", cov.cas);
    }
}
