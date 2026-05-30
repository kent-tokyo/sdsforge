//! Validation-driven correction pass.
//!
//! After the primary LLM extraction, [`apply_correction_pass`] looks at the
//! typed [`ValidationFinding`]s returned by [`super::validator::collect_findings`]
//! and applies targeted fixes:
//!
//! * **CAS check-digit mismatches** — fixed deterministically by recomputing the
//!   last digit.  No second LLM call is needed.
//! * **Unknown GHS H-codes / P-codes** — a single batched LLM call supplies
//!   the source text excerpt so the model can propose corrections based on
//!   what the original document actually says.
//!
//! The pass is **opt-in** (controlled by `ConvertConfig::correction`) and is
//! **fail-safe**: any error (LLM failure, JSON parse failure, response
//! validation failure) leaves the `SdsRoot` unchanged and appends a warning to
//! the notes vector — the overall conversion never fails because of the
//! correction pass.

use serde::Deserialize;
use tracing::warn;

use crate::ghs_codes;
use crate::schema::SdsRoot;

use super::llm::LlmBackend;
use super::validator::{PCodeCategory, ValidationFinding};

// ---------------------------------------------------------------------------
// Public configuration
// ---------------------------------------------------------------------------

/// Configuration for the optional validation-driven correction pass.
///
/// Currently no knobs are exposed; the struct exists so callers can opt in
/// via `ConvertConfig::correction = Some(CorrectionConfig::default())` and
/// future per-call overrides (e.g. model selection) can be added without a
/// breaking change.
#[derive(Debug, Clone, Default)]
pub struct CorrectionConfig {
    // Reserved for future extension (e.g. custom model override).
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Apply a validation-driven correction pass to `sds`.
///
/// Returns `(corrected_sds, notes)` where `notes` is a list of human-readable
/// messages describing each change made (or each error that prevented a
/// correction).  On total LLM failure the original `sds` is returned unchanged.
///
/// # Arguments
///
/// * `sds` — the SdsRoot produced by the primary LLM extraction.
/// * `source_text` — the raw text extracted from the source document, used
///   to supply a Section 2 excerpt to the LLM so it can ground its corrections.
/// * `findings` — output of [`super::validator::collect_findings`]; only
///   `UnknownHCode`, `UnknownPCode`, and `CasCheckDigit` variants are acted on.
/// * `backend` — any [`LlmBackend`] implementation.
/// * `_config` — reserved for future per-call options.
/// Return value of [`apply_correction_pass`].
pub struct CorrectionResult {
    pub sds: SdsRoot,
    pub notes: Vec<String>,
    /// CAS numbers that were deterministically corrected (new value after fix).
    /// `verify_against_source` skips these to avoid false-positive SourceVerify warnings.
    pub corrected_cas_values: Vec<String>,
}

pub async fn apply_correction_pass<B: LlmBackend + Sync>(
    mut sds: SdsRoot,
    source_text: &str,
    findings: &[ValidationFinding],
    backend: &B,
    _config: &CorrectionConfig,
) -> CorrectionResult {
    let mut notes: Vec<String> = Vec::new();
    let mut corrected_cas_values: Vec<String> = Vec::new();

    // ── Partition findings ────────────────────────────────────────────────────
    let cas_findings: Vec<&ValidationFinding> = findings
        .iter()
        .filter(|f| matches!(f, ValidationFinding::CasCheckDigit { .. }))
        .collect();
    let code_findings: Vec<&ValidationFinding> = findings
        .iter()
        .filter(|f| {
            matches!(
                f,
                ValidationFinding::UnknownHCode { .. } | ValidationFinding::UnknownPCode { .. }
            )
        })
        .collect();

    // ── CAS: deterministic fix ────────────────────────────────────────────────
    for finding in &cas_findings {
        if let ValidationFinding::CasCheckDigit {
            cas,
            composition_index,
            expected_digit,
        } = finding
        {
            if let Some(comp) = &mut sds.composition {
                if let Some(items) = &mut comp.composition_and_concentration {
                    if let Some(item) = items.get_mut(*composition_index) {
                        if let Some(ids) = &mut item.substance_identifiers {
                            if let Some(identity) = &mut ids.substance_identity {
                                if let Some(cas_node) = &mut identity.ca_sno {
                                    if let Some(texts) = &mut cas_node.full_text {
                                        for text in texts.iter_mut() {
                                            if text == cas {
                                                let fixed = fix_cas_check_digit(cas, *expected_digit);
                                                notes.push(format!(
                                                    "CAS corrected: '{cas}' → '{fixed}' \
                                                     (composition[{composition_index}])"
                                                ));
                                                corrected_cas_values.push(fixed.clone());
                                                *text = fixed;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // ── H/P-codes: LLM batch correction ──────────────────────────────────────
    if !code_findings.is_empty() {
        match query_code_corrections(source_text, &code_findings, backend).await {
            Ok(corrections) => {
                apply_code_corrections(&mut sds, &code_findings, &corrections, &mut notes);
            }
            Err(e) => {
                warn!("correction pass: LLM call failed — {e}");
                notes.push(format!(
                    "Correction pass: LLM call failed ({e}); H/P-code corrections skipped."
                ));
            }
        }
    }

    CorrectionResult { sds, notes, corrected_cas_values }
}

// ---------------------------------------------------------------------------
// CAS deterministic fix
// ---------------------------------------------------------------------------

/// Replace the check digit of a CAS number with `expected_digit`.
///
/// Assumes `cas` has already been validated as format-correct (i.e. it ends
/// with exactly one digit character).
fn fix_cas_check_digit(cas: &str, expected_digit: u32) -> String {
    debug_assert!(cas.ends_with(|c: char| c.is_ascii_digit()));
    let base = &cas[..cas.len().saturating_sub(1)];
    format!("{base}{expected_digit}")
}

// ---------------------------------------------------------------------------
// H/P-code LLM batch correction
// ---------------------------------------------------------------------------

/// One entry in the correction response array returned by the LLM.
#[derive(Debug, Deserialize)]
struct CorrectionEntry {
    original: String,
    /// `null` means the LLM could not find a valid replacement — delete entry.
    corrected: Option<String>,
    #[allow(dead_code)]
    reason: Option<String>,
}

/// Call the LLM once with all invalid H/P codes and parse a JSON array of
/// [`CorrectionEntry`] values.
async fn query_code_corrections<B: LlmBackend + Sync>(
    source_text: &str,
    findings: &[&ValidationFinding],
    backend: &B,
) -> Result<Vec<CorrectionEntry>, String> {
    let excerpt = extract_section2_excerpt(source_text);

    // Build a compact JSON array of the invalid codes with context.
    let mut invalid_items = Vec::new();
    for f in findings {
        match f {
            ValidationFinding::UnknownHCode { code, full_text, .. } => {
                let item = serde_json::json!({
                    "type": "H-code",
                    "invalid": code,
                    "full_text": full_text.as_deref().unwrap_or("")
                });
                invalid_items.push(item);
            }
            ValidationFinding::UnknownPCode { code, full_text, category, .. } => {
                let item = serde_json::json!({
                    "type": format!("P-code ({})", category),
                    "invalid": code,
                    "full_text": full_text.as_deref().unwrap_or("")
                });
                invalid_items.push(item);
            }
            _ => {}
        }
    }

    let invalid_json = serde_json::to_string(&invalid_items)
        .map_err(|e| format!("serialize invalid codes: {e}"))?;

    let system = "You are a GHS hazard/precautionary code validator. \
Respond ONLY with a valid JSON array — no markdown, no prose, no code fences.";

    let user = format!(
        "Source document excerpt (Section 2, ≤500 chars):\n\
<excerpt>\n{excerpt}\n</excerpt>\n\n\
The following GHS codes were extracted but are not in the official GHS code list:\n\
{invalid_json}\n\n\
For each invalid code, reply with exactly one JSON object:\n\
  {{\"original\": \"<invalid>\", \"corrected\": \"<valid GHS code>\", \"reason\": \"<brief>\"}}\n\
If no valid GHS code can be determined from the document, use null for \"corrected\".\n\
Reply ONLY with the JSON array."
    );

    let raw = backend.complete(system, &user).await.map_err(|e| e.to_string())?;
    parse_correction_response(&raw)
}

/// Parse the LLM correction response into a `Vec<CorrectionEntry>`.
fn parse_correction_response(raw: &str) -> Result<Vec<CorrectionEntry>, String> {
    // Strip optional markdown code fences.
    let trimmed = raw.trim();
    let json_str = if trimmed.starts_with("```") {
        trimmed
            .lines()
            .skip(1) // skip opening ```
            .take_while(|l| !l.starts_with("```")) // up to closing ```
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        trimmed.to_string()
    };

    // Find the JSON array.
    let start = json_str.find('[').ok_or_else(|| "no '[' in LLM response".to_string())?;
    let end = json_str.rfind(']').ok_or_else(|| "no ']' in LLM response".to_string())?;
    if end < start {
        return Err("malformed JSON array in LLM response".to_string());
    }

    let slice = &json_str[start..=end];
    serde_json::from_str::<Vec<CorrectionEntry>>(slice)
        .map_err(|e| format!("parse correction JSON: {e} — raw: {slice}"))
}

/// Extract up to 500 characters from the Section 2 area of the source text.
fn extract_section2_excerpt(source_text: &str) -> String {
    // Try to find a Section 2 heading (Japanese, English, or Chinese).
    let section2_markers = [
        "第2", "Section 2", "2.", "危険有害性の要約",
        "Hazard Identification", "危险性概述",
        "危害辨識",
    ];
    let section3_markers = [
        "第3", "Section 3", "3.", "組成", "Composition", "成分",
    ];

    let lower = source_text;
    let start_pos = section2_markers
        .iter()
        .filter_map(|marker| lower.find(marker))
        .min()
        .unwrap_or(0);

    // Find where Section 3 starts (to limit the excerpt).
    let end_pos = section3_markers
        .iter()
        .filter_map(|marker| {
            lower[start_pos..]
                .find(marker)
                .map(|p| p + start_pos)
        })
        .filter(|&p| p > start_pos + 10) // avoid matching the same heading
        .min()
        .unwrap_or_else(|| (start_pos + 1000).min(source_text.len()));

    let raw_excerpt = &source_text[start_pos..end_pos];
    // Limit to 500 bytes, but never split a multi-byte character.
    if raw_excerpt.len() <= 500 {
        raw_excerpt.to_string()
    } else {
        let mut cut = 500;
        while !raw_excerpt.is_char_boundary(cut) {
            cut -= 1;
        }
        raw_excerpt[..cut].to_string()
    }
}

// ---------------------------------------------------------------------------
// Apply LLM corrections to the SdsRoot
// ---------------------------------------------------------------------------

/// Apply the parsed [`CorrectionEntry`] list to the `sds`, guided by the
/// original `findings` to navigate to the right field paths.
fn apply_code_corrections(
    sds: &mut SdsRoot,
    findings: &[&ValidationFinding],
    corrections: &[CorrectionEntry],
    notes: &mut Vec<String>,
) {
    // Build a lookup: original_code → Option<corrected_code>
    // (last entry wins if duplicates).
    use std::collections::HashMap;
    let mut lookup: HashMap<&str, Option<&str>> = HashMap::new();
    for entry in corrections {
        lookup.insert(
            entry.original.as_str(),
            entry.corrected.as_deref(),
        );
    }

    for finding in findings {
        match finding {
            ValidationFinding::UnknownHCode {
                code,
                statement_index,
                ..
            } => {
                let Some(corrected_opt) = lookup.get(code.as_str()) else {
                    notes.push(format!("H-code '{code}': not in LLM correction response; kept as-is."));
                    continue;
                };

                match corrected_opt {
                    None => {
                        // LLM says delete.
                        if let Some(hz) = &mut sds.hazard_identification {
                            if let Some(hl) = &mut hz.hazard_labelling {
                                if let Some(stmts) = &mut hl.hazard_statement {
                                    if *statement_index < stmts.len() {
                                        notes.push(format!(
                                            "H-code '{code}' deleted from HazardStatement[{statement_index}] \
                                             (LLM: no valid replacement found)"
                                        ));
                                        stmts.remove(*statement_index);
                                    }
                                }
                            }
                        }
                    }
                    Some(new_code) => {
                        let upper = new_code.to_uppercase();
                        if !ghs_codes::is_valid_h_code(&upper) {
                            notes.push(format!(
                                "H-code '{code}': LLM suggested '{new_code}' which is also invalid; kept original."
                            ));
                            continue;
                        }
                        if let Some(hz) = &mut sds.hazard_identification {
                            if let Some(hl) = &mut hz.hazard_labelling {
                                if let Some(stmts) = &mut hl.hazard_statement {
                                    if let Some(stmt) = stmts.get_mut(*statement_index) {
                                        notes.push(format!(
                                            "H-code corrected: '{code}' → '{upper}' at HazardStatement[{statement_index}]"
                                        ));
                                        stmt.hazard_statement_code = Some(upper);
                                    }
                                }
                            }
                        }
                    }
                }
            }

            ValidationFinding::UnknownPCode {
                code,
                category,
                statement_index,
                ..
            } => {
                let Some(corrected_opt) = lookup.get(code.as_str()) else {
                    notes.push(format!("P-code '{code}': not in LLM correction response; kept as-is."));
                    continue;
                };

                match corrected_opt {
                    None => {
                        // LLM says delete.
                        apply_p_code_delete(sds, code, category, *statement_index, notes);
                    }
                    Some(new_code) => {
                        let upper = new_code.to_uppercase();
                        if !ghs_codes::is_valid_p_code(&upper) {
                            notes.push(format!(
                                "P-code '{code}': LLM suggested '{new_code}' which is also invalid; kept original."
                            ));
                            continue;
                        }
                        apply_p_code_replace(sds, code, category, *statement_index, &upper, notes);
                    }
                }
            }

            ValidationFinding::CasCheckDigit { .. } => {
                // Already handled deterministically above.
            }
        }
    }
}

/// Delete a P-code entry from the appropriate sub-list.
///
/// Each sub-list has a distinct concrete element type, so we use a macro to
/// avoid repeating the remove logic four times.
fn apply_p_code_delete(
    sds: &mut SdsRoot,
    code: &str,
    category: &PCodeCategory,
    idx: usize,
    notes: &mut Vec<String>,
) {
    let Some(hz) = &mut sds.hazard_identification else { return };
    let Some(hl) = &mut hz.hazard_labelling else { return };
    let Some(ps) = &mut hl.precautionary_statements else { return };

    macro_rules! do_delete {
        ($list_field:expr) => {
            if let Some(list) = $list_field.as_mut() {
                if idx < list.len() {
                    notes.push(format!(
                        "P-code '{code}' deleted from PrecautionaryStatements.{category}[{idx}] \
                         (LLM: no valid replacement found)"
                    ));
                    list.remove(idx);
                }
            }
        };
    }

    match category {
        PCodeCategory::Prevention => do_delete!(ps.prevention),
        PCodeCategory::Response   => do_delete!(ps.response),
        PCodeCategory::Storage    => do_delete!(ps.storage),
        PCodeCategory::Disposal   => do_delete!(ps.disposal),
    }
}

/// Replace a P-code value in the appropriate sub-list.
///
/// Each sub-list has a distinct concrete element type, so we use a macro to
/// avoid repeating the field-update logic four times.
fn apply_p_code_replace(
    sds: &mut SdsRoot,
    old_code: &str,
    category: &PCodeCategory,
    idx: usize,
    new_code: &str,
    notes: &mut Vec<String>,
) {
    let Some(hz) = &mut sds.hazard_identification else { return };
    let Some(hl) = &mut hz.hazard_labelling else { return };
    let Some(ps) = &mut hl.precautionary_statements else { return };

    macro_rules! do_replace {
        ($list_field:expr) => {
            if let Some(list) = $list_field.as_mut() {
                if let Some(entry) = list.get_mut(idx) {
                    notes.push(format!(
                        "P-code corrected: '{old_code}' → '{new_code}' \
                         at PrecautionaryStatements.{category}[{idx}]"
                    ));
                    entry.precautionary_statement_code = Some(new_code.to_string());
                }
            }
        };
    }

    match category {
        PCodeCategory::Prevention => do_replace!(ps.prevention),
        PCodeCategory::Response   => do_replace!(ps.response),
        PCodeCategory::Storage    => do_replace!(ps.storage),
        PCodeCategory::Disposal   => do_replace!(ps.disposal),
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fix_cas_check_digit_replaces_last_digit() {
        assert_eq!(fix_cas_check_digit("238016-30-4", 3), "238016-30-3");
        assert_eq!(fix_cas_check_digit("64-17-5", 5), "64-17-5");
        assert_eq!(fix_cas_check_digit("7732-18-5", 5), "7732-18-5");
        assert_eq!(fix_cas_check_digit("7732-18-9", 5), "7732-18-5");
    }

    #[test]
    fn test_parse_correction_response_valid() {
        let raw = r#"[{"original":"P286","corrected":"P285","reason":"typo"},{"original":"H999","corrected":null,"reason":"not found"}]"#;
        let entries = parse_correction_response(raw).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].original, "P286");
        assert_eq!(entries[0].corrected.as_deref(), Some("P285"));
        assert_eq!(entries[1].original, "H999");
        assert!(entries[1].corrected.is_none());
    }

    #[test]
    fn test_parse_correction_response_with_fences() {
        let raw = "```json\n[{\"original\":\"P289\",\"corrected\":\"P280\",\"reason\":\"closest match\"}]\n```";
        let entries = parse_correction_response(raw).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].corrected.as_deref(), Some("P280"));
    }

    #[test]
    fn test_extract_section2_excerpt_english() {
        let text = "Product Name: Ethanol\n\
Section 1. Identification\n...\n\
Section 2. Hazard Identification\nFlammable liquid\nH225, H302\n\
Section 3. Composition\n...";
        let excerpt = extract_section2_excerpt(text);
        assert!(excerpt.contains("Hazard Identification") || excerpt.contains("H225"));
        assert!(excerpt.len() <= 500);
    }
}
