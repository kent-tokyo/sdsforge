//! Projects a [`GenerationResult`] into the three artifacts a `generate`
//! invocation actually publishes: the official MHLW SDS JSON, a
//! machine-readable generation report, and a human-readable Markdown review
//! report.
//!
//! Pure and filesystem-free — this module only decides what bytes to write,
//! never where. Filesystem output belongs to the CLI/task layer (see
//! `sdsforge::tasks::run_generate`).
//!
//! `GenerationResult` already keeps `sds` (the official draft) strictly
//! separate from `findings`/`unresolved`/`provenance`/`evidence_summary`/
//! `release_status` (everything that explains the draft). This module does
//! not blur that boundary: [`serialize_official_sds`] touches only `sds`,
//! [`GenerationReport`] never embeds `SdsRoot`.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::converter::prune_empty_fields;
use crate::converter::validator::Finding;

use super::provenance::{EvidenceLevel, FieldProvenance};
use super::result::{
    compute_evidence_summary, compute_release_status, evaluate_release_gate, EvidenceSummary,
    GenerationResult, ReleaseGateResult, ReleaseStatus,
};
use super::unresolved::{UnresolvedField, UnresolvedReason};

/// Version of the `generation_report.json` *shape*, independent of the
/// crate version — the report format and the crate can each change without
/// the other. No timestamp is embedded by default: repeated serialization
/// of the same [`GenerationResult`] must stay byte-equivalent.
pub const REPORT_SCHEMA_VERSION: &str = "1.0";

/// Everything needed to understand an SDS draft's completeness, without the
/// draft itself. Never embeds `SdsRoot` — see `tests::
/// generation_report_json_does_not_contain_full_sds`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationReport {
    pub report_schema_version: String,
    pub release_status: ReleaseStatus,
    pub findings: Vec<Finding>,
    pub unresolved: Vec<UnresolvedField>,
    pub provenance: Vec<FieldProvenance>,
    pub evidence_summary: EvidenceSummary,
    pub release_gate: ReleaseGateResult,
}

/// Builds a [`GenerationReport`] from a [`GenerationResult`].
///
/// `evidence_summary`/`release_status` are recomputed from
/// `result.provenance`/`result.unresolved`/`result.findings` rather than
/// copied from `result` as-is — a caller could have hand-built or edited a
/// `GenerationResult`, and the report must never trust stale counters. The
/// recomputed values feed [`evaluate_release_gate`] so `release_gate.status`
/// can never disagree with `release_status` above it.
pub fn build_generation_report(result: &GenerationResult) -> GenerationReport {
    let evidence_summary = compute_evidence_summary(&result.provenance, &result.unresolved);
    let release_status = compute_release_status(&result.unresolved, &result.findings);

    let mut gated = result.clone();
    gated.release_status = release_status;
    let release_gate = evaluate_release_gate(&gated);

    GenerationReport {
        report_schema_version: REPORT_SCHEMA_VERSION.to_string(),
        release_status,
        findings: result.findings.clone(),
        unresolved: result.unresolved.clone(),
        provenance: result.provenance.clone(),
        evidence_summary,
        release_gate,
    }
}

#[derive(Debug, Error)]
pub enum GenerationArtifactError {
    #[error("failed to serialize official SDS JSON: {0}")]
    OfficialSdsSerialization(#[source] serde_json::Error),
    #[error("failed to serialize generation report JSON: {0}")]
    ReportSerialization(#[source] serde_json::Error),
    #[error("invalid artifact state: {0}")]
    InvalidArtifactState(String),
}

/// Serializes only `result.sds` — the same serialize→[`prune_empty_fields`]→
/// pretty-print path the existing `to-json`/`render` commands already use,
/// so `official_sds.json` matches the formatting of every other JSON this
/// crate writes. No report metadata, evidence references, or normalization
/// diagnostics are ever added.
pub fn serialize_official_sds(
    result: &GenerationResult,
) -> Result<String, GenerationArtifactError> {
    let value = serde_json::to_value(&result.sds)
        .map_err(GenerationArtifactError::OfficialSdsSerialization)?;
    let pruned = prune_empty_fields(value);
    serde_json::to_string_pretty(&pruned).map_err(GenerationArtifactError::OfficialSdsSerialization)
}

/// Serializes a [`GenerationReport`] as pretty JSON. Contains only what
/// [`GenerationReport`] itself carries — no API keys, HTTP headers, evidence
/// binaries, temp-file paths, or ad hoc debug formatting ever enter this
/// type, so there is nothing to redact here.
pub fn serialize_generation_report(
    report: &GenerationReport,
) -> Result<String, GenerationArtifactError> {
    serde_json::to_string_pretty(report).map_err(GenerationArtifactError::ReportSerialization)
}

/// The three artifacts a `generate` invocation publishes, as ready-to-write
/// strings. Keeps filesystem concerns out of `sdsforge-core` while giving
/// Rust callers the same three outputs the CLI writes to disk.
pub struct GenerationArtifacts {
    pub official_sds_json: String,
    pub generation_report_json: String,
    pub review_report_markdown: String,
}

pub fn build_generation_artifacts(
    result: &GenerationResult,
) -> Result<GenerationArtifacts, GenerationArtifactError> {
    let official_sds_json = serialize_official_sds(result)?;
    let report = build_generation_report(result);
    let generation_report_json = serialize_generation_report(&report)?;
    let review_report_markdown = render_review_report(&report);
    Ok(GenerationArtifacts {
        official_sds_json,
        generation_report_json,
        review_report_markdown,
    })
}

// ---------------------------------------------------------------------------
// Markdown review report
// ---------------------------------------------------------------------------

/// Renders a deterministic, human-readable Markdown summary of a
/// [`GenerationReport`]. Never uses an LLM. Always states plainly that the
/// output is an unapproved automated draft — even a (currently
/// unreachable-by-generation-code) `ReleaseStatus::Approved` still carries
/// that disclaimer, so the wording can never be read as a real approval.
pub fn render_review_report(report: &GenerationReport) -> String {
    let mut out = String::new();

    out.push_str("# SDS Generation Review Report\n\n");

    out.push_str("## Release status\n\n");
    out.push_str(describe_release_status(report.release_status));
    out.push_str("\n\n");
    out.push_str(
        "This output is an automatically generated SDS draft. It has not been \
         approved by a qualified reviewer.\n\n",
    );

    let blocking_unresolved = report
        .unresolved
        .iter()
        .filter(|f| f.blocks_release)
        .count();
    out.push_str("## Summary\n\n");
    out.push_str(&format!("- Findings: {}\n", report.findings.len()));
    out.push_str(&format!(
        "- Blocking findings: {}\n",
        report.release_gate.blocking_findings.len()
    ));
    out.push_str(&format!(
        "- Unresolved fields: {}\n",
        report.unresolved.len()
    ));
    out.push_str(&format!(
        "- Unresolved fields blocking release: {}\n",
        blocking_unresolved
    ));
    out.push_str(&format!(
        "- Confirmed fields: {}\n",
        report.evidence_summary.confirmed
    ));
    out.push_str(&format!(
        "- Unverified user-input fields: {}\n",
        report.evidence_summary.unverified_user_input
    ));
    out.push('\n');

    out.push_str("## Blocking issues\n\n");
    render_findings_list(
        &mut out,
        &sorted_findings(&report.release_gate.blocking_findings),
    );

    out.push_str("## Required actions\n\n");
    if report.release_gate.required_actions.is_empty() {
        out.push_str("None.\n\n");
    } else {
        for action in &report.release_gate.required_actions {
            out.push_str(&format!("- {}\n", esc(action)));
        }
        out.push('\n');
    }

    out.push_str("## Unresolved fields\n\n");
    if report.unresolved.is_empty() {
        out.push_str("None.\n\n");
    } else {
        let mut unresolved = report.unresolved.clone();
        unresolved.sort_by(|a, b| a.path.cmp(&b.path));
        for field in &unresolved {
            render_unresolved_field(&mut out, field);
        }
    }

    out.push_str("## Findings\n\n");
    render_findings_list(&mut out, &sorted_findings(&report.findings));

    out.push_str("## Evidence summary\n\n");
    let s = &report.evidence_summary;
    out.push_str(&format!("- Confirmed: {}\n", s.confirmed));
    out.push_str(&format!("- Supplied: {}\n", s.supplied));
    out.push_str(&format!("- Literature: {}\n", s.literature));
    out.push_str(&format!("- Calculated: {}\n", s.calculated));
    out.push_str(&format!("- Estimated: {}\n", s.estimated));
    out.push_str(&format!("- Unresolved: {}\n", s.unresolved));
    out.push_str(&format!("- Not applicable: {}\n", s.not_applicable));
    out.push_str(&format!(
        "- Product test evidence: {}\n",
        s.product_test_evidence
    ));
    out.push_str(&format!(
        "- Unverified user input: {}\n",
        s.unverified_user_input
    ));
    out.push('\n');

    out.push_str("## Provenance\n\n");
    if report.provenance.is_empty() {
        out.push_str("None.\n\n");
    } else {
        for p in &report.provenance {
            render_provenance_entry(&mut out, p);
        }
        out.push('\n');
    }

    out
}

fn render_findings_list(out: &mut String, findings: &[Finding]) {
    if findings.is_empty() {
        out.push_str("None.\n\n");
        return;
    }
    for f in findings {
        out.push_str(&format!(
            "- **[{}] {}**: {}\n",
            esc(&f.level),
            esc(&f.rule),
            esc(&f.message)
        ));
    }
    out.push('\n');
}

fn sorted_findings(findings: &[Finding]) -> Vec<Finding> {
    let mut findings = findings.to_vec();
    findings.sort_by_key(finding_sort_key);
    findings
}

fn finding_sort_key(f: &Finding) -> (u8, String, String) {
    let rank = match f.level.as_str() {
        "CRIT" => 0,
        "HIGH" => 1,
        "MED" => 2,
        "LOW" => 3,
        "WARN" => 4,
        _ => 5,
    };
    (rank, f.rule.clone(), f.message.clone())
}

fn render_unresolved_field(out: &mut String, field: &UnresolvedField) {
    out.push_str(&format!("### {}\n\n", esc(&field.title)));
    out.push_str(&format!("- Path: `{}`\n", field.path));
    out.push_str(&format!("- Reason: {}\n", describe_reason(field.reason)));
    out.push_str(&format!("- Blocks release: {}\n", field.blocks_release));
    out.push_str(&format!("- Safety impact: {:?}\n", field.safety_impact));
    out.push_str(&format!(
        "- Regulatory impact: {:?}\n",
        field.regulatory_impact
    ));
    if !field.required_inputs.is_empty() {
        out.push_str("- Required inputs:\n");
        for input in &field.required_inputs {
            match &input.unit {
                Some(unit) => out.push_str(&format!(
                    "  - {} ({}): {}\n",
                    esc(&input.name),
                    esc(unit),
                    esc(&input.description)
                )),
                None => out.push_str(&format!(
                    "  - {}: {}\n",
                    esc(&input.name),
                    esc(&input.description)
                )),
            }
        }
    }
    if !field.acceptable_evidence.is_empty() {
        out.push_str("- Acceptable evidence:\n");
        for level in &field.acceptable_evidence {
            out.push_str(&format!("  - {}\n", describe_evidence_level(*level)));
        }
    }
    out.push_str(&format!(
        "- Recommended action: {}\n",
        esc(&field.recommended_action)
    ));
    out.push('\n');
}

fn render_provenance_entry(out: &mut String, p: &FieldProvenance) {
    out.push_str(&format!(
        "- `{}` — {:?}, confidence {:?}",
        p.path, p.source_type, p.confidence
    ));
    if let Some(reference) = &p.source_reference {
        out.push_str(&format!(", reference: {}", esc(reference)));
    }
    if let Some(value) = &p.source_value {
        out.push_str(&format!(", value: {}", esc(value)));
    }
    if !p.method.is_empty() {
        out.push_str(&format!(", method: {}", esc(&p.method)));
    }
    if let Some(sample_id) = &p.sample_id {
        out.push_str(&format!(", sample: {}", esc(sample_id)));
    }
    if let Some(batch_id) = &p.batch_id {
        out.push_str(&format!(", batch: {}", esc(batch_id)));
    }
    if let Some(test_method) = &p.test_method {
        out.push_str(&format!(", test method: {}", esc(test_method)));
    }
    if let Some(retrieved_at) = &p.retrieved_at {
        out.push_str(&format!(", retrieved: {}", esc(retrieved_at)));
    }
    if !p.warnings.is_empty() {
        let warnings: Vec<String> = p.warnings.iter().map(|w| esc(w)).collect();
        out.push_str(&format!(", warnings: {}", warnings.join("; ")));
    }
    out.push('\n');
}

fn describe_release_status(status: ReleaseStatus) -> &'static str {
    match status {
        ReleaseStatus::Draft => "DRAFT",
        ReleaseStatus::ReviewRequired => "REVIEW REQUIRED",
        ReleaseStatus::Blocked => "BLOCKED",
        ReleaseStatus::Approved => "APPROVED (AUTOMATED DRAFT — STILL REQUIRES HUMAN SIGN-OFF)",
    }
}

fn describe_reason(reason: UnresolvedReason) -> &'static str {
    match reason {
        UnresolvedReason::MissingInput => "Missing input",
        UnresolvedReason::ProductTestRequired => "Product test required",
        UnresolvedReason::AmbiguousChemicalIdentity => "Ambiguous chemical identity",
        UnresolvedReason::ConflictingSources => "Conflicting sources",
        UnresolvedReason::UnsupportedCalculation => "Unsupported calculation",
        UnresolvedReason::InsufficientMeasurementConditions => {
            "Insufficient measurement conditions"
        }
        UnresolvedReason::MixtureCannotBeDerivedFromComponents => {
            "Mixture property cannot be derived from component values"
        }
        UnresolvedReason::RegulatoryJudgementRequired => "Regulatory judgement required",
        UnresolvedReason::HumanReviewRequired => "Human review required",
    }
}

fn describe_evidence_level(level: EvidenceLevel) -> &'static str {
    match level {
        EvidenceLevel::ProductTestReport => "product test report",
        EvidenceLevel::EquivalentBatchTestReport => "equivalent-batch test report",
        EvidenceLevel::SupplierSpecification => "supplier specification",
        EvidenceLevel::SupplierSds => "supplier SDS",
        EvidenceLevel::RegulatoryDatabase => "regulatory database",
        EvidenceLevel::PeerReviewedLiterature => "peer-reviewed literature",
        EvidenceLevel::ReferenceDatabase => "reference database",
        EvidenceLevel::DeterministicCalculation => "deterministic calculation",
        EvidenceLevel::ModelEstimate => "model estimate",
        EvidenceLevel::UnverifiedUserInput => "unverified user input",
        EvidenceLevel::None => "none",
    }
}

/// Neutralizes the Markdown/HTML control characters that could otherwise
/// let user-supplied text (sample IDs, evidence references, measurement
/// methods — anything traced back to `ProductInput`) break the report's
/// structure or inject raw HTML: embedded newlines are collapsed so text
/// can never start a new line (defusing headings/list markers/table rows
/// at once), and the remaining structurally significant characters are
/// backslash-escaped.
fn esc(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '\n' | '\r' => out.push(' '),
            '\\' | '`' | '*' | '_' | '[' | ']' | '<' | '>' | '|' => {
                out.push('\\');
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generation::input::ProductInput;
    use crate::generation::provenance::{path, ConfidenceLevel};
    use crate::generation::result::generate_from_resolved_input;
    use crate::generation::unresolved::{RegulatoryImpact, RequiredInput, SafetyImpact};
    use std::collections::HashMap;

    fn empty_result() -> GenerationResult {
        generate_from_resolved_input(&ProductInput::default(), &HashMap::new())
    }

    fn with_high_finding(rule: &str, message: &str) -> GenerationResult {
        let mut result = empty_result();
        result.findings.push(Finding {
            level: "HIGH".into(),
            rule: rule.into(),
            message: message.into(),
        });
        result.release_status = compute_release_status(&result.unresolved, &result.findings);
        result
    }

    fn sample_unresolved(path: &str, recommended_action: &str) -> UnresolvedField {
        UnresolvedField {
            path: path.into(),
            title: "Flash point".into(),
            reason: UnresolvedReason::ProductTestRequired,
            required_inputs: vec![
                RequiredInput::new("measured value", "The measured flash point").with_unit("°C"),
                RequiredInput::new("test method", "The test method used"),
            ],
            acceptable_evidence: vec![
                EvidenceLevel::ProductTestReport,
                EvidenceLevel::EquivalentBatchTestReport,
            ],
            safety_impact: SafetyImpact::High,
            regulatory_impact: RegulatoryImpact::High,
            recommended_action: recommended_action.into(),
            blocks_release: true,
        }
    }

    // -- official_sds.json purity ------------------------------------------

    #[test]
    fn official_sds_json_contains_only_sds_root_fields() {
        let mut result = with_high_finding("GEN-TEST", "a HIGH finding");
        result.unresolved.push(sample_unresolved(
            &path::composition_row(0, path::CAS_NO),
            "Obtain a product test report.",
        ));
        result.provenance.push(FieldProvenance::supplied(
            path::TRADE_NAME_JP,
            "user-supplied trade name",
        ));

        let json = serialize_official_sds(&result).unwrap();
        for leak in [
            "release_status",
            "findings",
            "unresolved",
            "provenance",
            "evidence_summary",
            "release_gate",
            "report_schema_version",
            "NormalizationStatus",
            "screening_alerts",
            "evidence_id",
            "source_reference",
            "confidence",
        ] {
            assert!(
                !json.contains(leak),
                "official SDS JSON must not contain '{leak}'"
            );
        }
    }

    #[test]
    fn empty_official_fields_are_pruned() {
        let result = empty_result();
        let json = serialize_official_sds(&result).unwrap();
        assert!(!json.contains("null"));
        assert!(!json.contains(r#""""#));
    }

    #[test]
    fn section_3_order_in_official_json_is_unchanged() {
        let mut input = ProductInput::default();
        input.trade_name = "Two Component Mix".into();
        input.components = vec![
            crate::generation::input::ComponentInput {
                cas_number: Some("7732-18-5".into()),
                name: Some("Water".into()),
                concentration: crate::generation::input::ConcentrationRange {
                    exact: Some(90.0),
                    lower: None,
                    upper: None,
                    unit: "%".into(),
                },
            },
            crate::generation::input::ComponentInput {
                cas_number: Some("64-17-5".into()),
                name: Some("Ethanol".into()),
                concentration: crate::generation::input::ConcentrationRange {
                    exact: Some(10.0),
                    lower: None,
                    upper: None,
                    unit: "%".into(),
                },
            },
        ];
        let result = generate_from_resolved_input(&input, &HashMap::new());
        let json = serialize_official_sds(&result).unwrap();
        let water_pos = json.find("Water").unwrap();
        let ethanol_pos = json.find("Ethanol").unwrap();
        assert!(water_pos < ethanol_pos, "component order must be preserved");
    }

    // -- generation_report.json ----------------------------------------------

    #[test]
    fn generation_report_json_does_not_contain_full_sds() {
        // `path` strings legitimately reuse MHLW field names (e.g.
        // "Identification.TradeProductIdentity.TradeNameJP"), so a substring
        // check can't distinguish "provenance references this field" from
        // "SdsRoot was embedded". Check the top-level key set instead: it
        // must be exactly the report's own fields, never `sds`/`Identification`/
        // any other schema section key.
        let result = with_high_finding("GEN-TEST", "a HIGH finding");
        let report = build_generation_report(&result);
        let json = serialize_generation_report(&report).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        let keys: std::collections::BTreeSet<&str> = value
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        let expected: std::collections::BTreeSet<&str> = [
            "report_schema_version",
            "release_status",
            "findings",
            "unresolved",
            "provenance",
            "evidence_summary",
            "release_gate",
        ]
        .into_iter()
        .collect();
        assert_eq!(keys, expected);
    }

    #[test]
    fn generation_report_contains_blocking_findings_and_required_actions() {
        let mut result = with_high_finding("GEN-TEST", "a HIGH finding");
        result.unresolved.push(sample_unresolved(
            &path::composition_row(0, path::CAS_NO),
            "Obtain a product test report.",
        ));
        result.release_status = compute_release_status(&result.unresolved, &result.findings);
        let report = build_generation_report(&result);

        assert!(!report.release_gate.blocking_findings.is_empty());
        assert!(!report.release_gate.required_actions.is_empty());
        assert_eq!(report.release_status, ReleaseStatus::Blocked);
    }

    #[test]
    fn required_actions_are_deduplicated() {
        let mut result = empty_result();
        result
            .unresolved
            .push(sample_unresolved("path.a", "Obtain a product test report."));
        result
            .unresolved
            .push(sample_unresolved("path.b", "Obtain a product test report."));
        let report = build_generation_report(&result);
        assert_eq!(report.release_gate.required_actions.len(), 1);
    }

    #[test]
    fn all_provenance_details_remain_available_in_the_report() {
        use crate::generation::evidence::{EvidenceApplicability, EvidenceSource};
        use crate::generation::provenance::MeasurementConditions;

        let source = EvidenceSource {
            id: "ev1".into(),
            level: EvidenceLevel::ProductTestReport,
            reference: "Lab Report 2026-014".into(),
            issuer: None,
            document_date: None,
            applies_to: EvidenceApplicability::FinishedProduct,
        };
        let conditions = MeasurementConditions {
            temperature_c: None,
            pressure_kpa: None,
            atmosphere: None,
        };
        let mut result = empty_result();
        result
            .provenance
            .push(FieldProvenance::from_measured_evidence(
                path::composition_row(0, "FlashPoint"),
                &source,
                Some("Closed Cup (ASTM D93)"),
                Some("sample-42"),
                None,
                &conditions,
            ));
        let report = build_generation_report(&result);
        let json = serialize_generation_report(&report).unwrap();
        assert!(json.contains("sample-42"));
        assert!(json.contains("ASTM D93"));
    }

    // -- Markdown review report -----------------------------------------------

    #[test]
    fn markdown_report_begins_with_draft_warning() {
        let report = build_generation_report(&empty_result());
        let md = render_review_report(&report);
        assert!(md.starts_with("# SDS Generation Review Report\n"));
        let summary_pos = md.find("## Summary").unwrap();
        let warning_pos = md.find("has not been approved").unwrap();
        assert!(
            warning_pos < summary_pos,
            "draft warning must appear before the summary"
        );
    }

    #[test]
    fn blocked_status_is_visibly_rendered_as_blocked() {
        let result = with_high_finding("GEN-TEST", "a HIGH finding");
        let report = build_generation_report(&result);
        let md = render_review_report(&report);
        assert!(md.contains("BLOCKED"));
    }

    #[test]
    fn review_required_status_is_visibly_rendered() {
        let mut result = empty_result();
        result.findings.push(Finding {
            level: "LOW".into(),
            rule: "GEN-TEST".into(),
            message: "a LOW finding".into(),
        });
        result.release_status = compute_release_status(&result.unresolved, &result.findings);
        let report = build_generation_report(&result);
        let md = render_review_report(&report);
        assert!(md.contains("REVIEW REQUIRED"));
    }

    #[test]
    fn approved_status_still_carries_draft_warning() {
        let mut report = build_generation_report(&empty_result());
        report.release_status = ReleaseStatus::Approved;
        let md = render_review_report(&report);
        assert!(md.contains("has not been approved"));
        assert!(!md.to_uppercase().contains("APPROVED\n"));
    }

    #[test]
    fn formula_mismatch_remains_blocking_in_all_report_forms() {
        let result = with_high_finding(
            "GEN-STRUCTURE-FORMULA-MISMATCH",
            "resolver formula C2H6O disagrees with calculated formula C6H12O6",
        );
        assert_eq!(result.release_status, ReleaseStatus::Blocked);
        let report = build_generation_report(&result);
        assert_eq!(report.release_status, ReleaseStatus::Blocked);
        assert!(report
            .release_gate
            .blocking_findings
            .iter()
            .any(|f| f.rule == "GEN-STRUCTURE-FORMULA-MISMATCH"));
        let md = render_review_report(&report);
        assert!(md.contains("BLOCKED"));
        assert!(md.contains("GEN-STRUCTURE-FORMULA-MISMATCH"));
    }

    #[test]
    fn multi_fragment_review_warning_is_nonblocking_by_itself() {
        let mut result = empty_result();
        result.findings.push(Finding {
            level: "MED".into(),
            rule: "GEN-STRUCTURE-MULTIFRAGMENT".into(),
            message: "structure has multiple disconnected fragments".into(),
        });
        result.release_status = compute_release_status(&result.unresolved, &result.findings);
        assert_eq!(result.release_status, ReleaseStatus::ReviewRequired);
        let report = build_generation_report(&result);
        assert!(report.release_gate.blocking_findings.is_empty());
        let md = render_review_report(&report);
        assert!(!md.contains("BLOCKED"));
        assert!(md.contains("REVIEW REQUIRED"));
    }

    #[test]
    fn user_supplied_content_cannot_inject_headings_or_raw_html() {
        let malicious = "\n## Fake Injected Heading\n<script>alert(1)</script>\n| break | table |";
        let mut result = empty_result();
        result.findings.push(Finding {
            level: "LOW".into(),
            rule: "GEN-TEST".into(),
            message: malicious.into(),
        });
        result.release_status = compute_release_status(&result.unresolved, &result.findings);
        let report = build_generation_report(&result);
        let md = render_review_report(&report);

        let heading_lines: Vec<&str> = md.lines().filter(|l| l.starts_with("## ")).collect();
        assert_eq!(
            heading_lines.len(),
            8,
            "no new top-level heading may be injected"
        );
        assert!(!md.contains("<script>"));
        assert!(md.contains("\\<script\\>"));
    }

    #[test]
    fn repeated_artifact_generation_is_byte_equivalent() {
        let mut result = with_high_finding("GEN-TEST", "a HIGH finding");
        result.provenance.push(FieldProvenance::supplied(
            path::TRADE_NAME_JP,
            "user-supplied trade name",
        ));
        let a = build_generation_artifacts(&result).unwrap();
        let b = build_generation_artifacts(&result).unwrap();
        assert_eq!(a.official_sds_json, b.official_sds_json);
        assert_eq!(a.generation_report_json, b.generation_report_json);
        assert_eq!(a.review_report_markdown, b.review_report_markdown);
    }

    #[test]
    fn confidence_level_serializes_snake_case_in_report() {
        let mut result = empty_result();
        result.provenance.push(FieldProvenance::supplied(
            path::TRADE_NAME_JP,
            "user-supplied trade name",
        ));
        let report = build_generation_report(&result);
        let json = serialize_generation_report(&report).unwrap();
        assert!(json.contains("\"unverified\""));
        let _ = ConfidenceLevel::Unverified;
    }
}
