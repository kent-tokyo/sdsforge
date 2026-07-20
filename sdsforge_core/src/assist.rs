//! Assist v1: proposes candidate values for Section 4 (First-aid measures)
//! fields, extracted from one supplier SDS document, for a human to
//! accept/edit/reject. Assist never writes to `official_sds.json` or
//! [`crate::generation::ProductInput`] directly -- turning an accepted
//! proposal into authoring input is a separate (not yet implemented) step.
//!
//! Reuses [`ConfidenceLevel`]/[`EvidenceLevel`] from [`crate::generation`]
//! rather than inventing assist-specific enums: an accepted proposal folds
//! straight into the same provenance model `generate` already uses.
//!
//! `EvidenceLevel` describes *what the source document is*, not how a
//! value was pulled out of it -- an LLM locating and quoting a paragraph
//! from a supplier SDS doesn't change that the evidence is still
//! `SupplierSds`. That's why `AssistRun` carries `source_evidence_level`
//! (fixed to `SupplierSds` in v1, since the CLI only accepts
//! `--source-kind supplier-sds`) and `extraction_method` (fixed to
//! `"llm_extraction"`) as two separate fields, instead of collapsing them
//! into a single `EvidenceLevel::ModelEstimate` -- that value is reserved
//! for genuinely model-*estimated* properties, not source-*extracted* ones.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::generation::{ConfidenceLevel, EvidenceLevel};

pub const ASSIST_SCHEMA_VERSION: &str = "1";

/// The only extraction method assist v1 has.
pub const EXTRACTION_METHOD_LLM: &str = "llm_extraction";

/// Confidence assist v1 ever assigns to an emitted proposal. A candidate
/// that fails any deterministic check in [`validate_candidate`] is
/// rejected outright, never downgraded to `Low` -- see that function.
pub const ASSIST_CONFIDENCE: ConfidenceLevel = ConfidenceLevel::Medium;

/// Section 4 (First-aid measures) dot-paths assist v1 may target -- the
/// same MHLW-JSON-key dot-path convention as
/// `generation::provenance::FieldProvenance::path`. Every path here is a
/// `FullText: Option<String>` leaf in [`crate::schema::SdsRoot`]; the
/// `Vec<String>` symptom-list fields under `InformationToHealthProfessionals`
/// are out of scope for v1 (see [`is_allowed_path`]).
pub const SECTION4_ALLOWED_PATHS: &[&str] = &[
    "FirstAidMeasures.ExposureRoute.FirstAidInhalation.FullText",
    "FirstAidMeasures.ExposureRoute.FirstAidSkin.FullText",
    "FirstAidMeasures.ExposureRoute.FirstAidEye.FullText",
    "FirstAidMeasures.ExposureRoute.FirstAidIngestion.FullText",
    "FirstAidMeasures.DescriptionOfFirstAidMeasures.FullText",
    "FirstAidMeasures.InformationToHealthProfessionals.FullText",
    "FirstAidMeasures.MedicalAttentionAndSpecialTreatmentNeeded.FullText",
];

pub fn is_allowed_path(path: &str) -> bool {
    SECTION4_ALLOWED_PATHS.contains(&path)
}

/// Raw model output for one candidate value -- exactly the fields the
/// assist prompt asks for and nothing else. `deny_unknown_fields` means a
/// candidate carrying a model-supplied `id`, `confidence`, `evidence_level`,
/// or any approval/release-status key fails to parse as this type at all;
/// [`build_proposals`] treats that the same as any other invalid candidate
/// (omitted, with a warning), not as a reason to abort the whole run.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssistCandidate {
    pub path: String,
    pub proposed_value: serde_json::Value,
    pub source_page: Option<u32>,
    pub source_excerpt: String,
    pub rationale: Option<String>,
}

/// One accepted candidate: allowlisted path, non-empty excerpt verified
/// against the source text, host-assigned deterministic `id`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssistProposal {
    pub id: String,
    pub path: String,
    pub proposed_value: serde_json::Value,
    pub source_page: Option<u32>,
    pub source_excerpt: String,
    pub confidence: ConfidenceLevel,
    pub rationale: Option<String>,
}

/// One assist run's output: a batch of proposals plus the document- and
/// model-level metadata that applies to all of them. v1 processes exactly
/// one source document per run, so that metadata is recorded once here
/// rather than repeated on every proposal.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssistRun {
    pub schema_version: String,

    pub source_document: String,
    pub source_sha256: String,
    pub source_evidence_level: EvidenceLevel,

    pub extraction_method: String,
    pub model_provider: String,
    pub model_name: String,
    pub prompt_version: String,

    pub proposals: Vec<AssistProposal>,
    pub warnings: Vec<String>,
}

/// Hex-encoded SHA-256 of `bytes` -- used both for `AssistRun::source_sha256`
/// (the whole source document) and, via [`proposal_id`], for deterministic
/// proposal ids.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

/// `assist-<12 hex chars>` derived from `source_sha256` plus the
/// candidate's own stable content (path, page, excerpt, value) -- never
/// taken from the model. The same source document and accepted model
/// output always produce the same id.
fn proposal_id(
    source_sha256: &str,
    path: &str,
    source_page: Option<u32>,
    source_excerpt: &str,
    proposed_value: &serde_json::Value,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(source_sha256.as_bytes());
    hasher.update(b"\0");
    hasher.update(path.as_bytes());
    hasher.update(b"\0");
    hasher.update(source_page.map(|p| p.to_string()).unwrap_or_default().as_bytes());
    hasher.update(b"\0");
    hasher.update(source_excerpt.as_bytes());
    hasher.update(b"\0");
    hasher.update(proposed_value.to_string().as_bytes());
    let digest = format!("{:x}", hasher.finalize());
    format!("assist-{}", &digest[..12])
}

/// Whether `excerpt` appears verbatim in `source_text`, ignoring
/// whitespace differences (PDF text extraction commonly reflows line
/// breaks). Assist must run this against every candidate's
/// `source_excerpt` before emitting a proposal -- an excerpt that doesn't
/// verify is a hallucinated citation, not a real one.
///
/// Deliberately just a whitespace-normalized substring check for v1: no
/// punctuation normalization, no OCR-error tolerance, no full/half-width or
/// other Unicode-variant folding. A real excerpt that differs from the
/// source by punctuation, an OCR misread, or a width variant will fail
/// this check and be rejected -- a conservative false negative, not a
/// false positive. Add fuzzy matching only once real documents show this
/// margin is actually too tight.
pub fn excerpt_verifies(source_text: &str, excerpt: &str) -> bool {
    let needle: String = excerpt.split_whitespace().collect::<Vec<_>>().join(" ");
    if needle.is_empty() {
        return false;
    }
    let haystack: String = source_text.split_whitespace().collect::<Vec<_>>().join(" ");
    haystack.contains(&needle)
}

/// Validates one raw model candidate against the Section 4 allowlist and
/// the extracted source text, returning the finished proposal or a
/// human-readable rejection reason.
pub fn validate_candidate(
    candidate: &AssistCandidate,
    source_sha256: &str,
    source_text: &str,
) -> Result<AssistProposal, String> {
    if !is_allowed_path(&candidate.path) {
        return Err(format!(
            "path '{}' is not in the Section 4 allowlist",
            candidate.path
        ));
    }
    let Some(value_str) = candidate.proposed_value.as_str() else {
        return Err(format!(
            "path '{}': proposed_value must be a string",
            candidate.path
        ));
    };
    if value_str.trim().is_empty() {
        return Err(format!("path '{}': proposed_value is empty", candidate.path));
    }
    if candidate.source_excerpt.trim().is_empty() {
        return Err(format!("path '{}': source_excerpt is empty", candidate.path));
    }
    if let Some(0) = candidate.source_page {
        return Err(format!(
            "path '{}': source_page must be positive",
            candidate.path
        ));
    }
    if !excerpt_verifies(source_text, &candidate.source_excerpt) {
        return Err(format!(
            "path '{}': source_excerpt not found in extracted source text",
            candidate.path
        ));
    }

    let id = proposal_id(
        source_sha256,
        &candidate.path,
        candidate.source_page,
        &candidate.source_excerpt,
        &candidate.proposed_value,
    );

    Ok(AssistProposal {
        id,
        path: candidate.path.clone(),
        proposed_value: candidate.proposed_value.clone(),
        source_page: candidate.source_page,
        source_excerpt: candidate.source_excerpt.clone(),
        confidence: ASSIST_CONFIDENCE,
        rationale: candidate.rationale.clone(),
    })
}

/// Parses the LLM's raw response as a JSON array of candidate objects.
/// Failure here means the response is malformed as a whole -- callers
/// should surface this as a hard error and write no output file, unlike a
/// single invalid candidate (see [`build_proposals`]).
pub fn parse_candidates_json(raw: &str) -> Result<Vec<serde_json::Value>, String> {
    let value: serde_json::Value =
        serde_json::from_str(raw).map_err(|e| format!("assist response is not valid JSON: {e}"))?;
    match value {
        serde_json::Value::Array(items) => Ok(items),
        _ => Err("assist response must be a JSON array of candidate objects".to_string()),
    }
}

/// Validates each raw candidate value independently: a candidate that
/// fails to parse as [`AssistCandidate`] or fails [`validate_candidate`]
/// is omitted with a warning, never aborts the batch. Call
/// [`parse_candidates_json`] first to get `raw_candidates` -- a malformed
/// top-level response is a separate, harder failure (see that function).
pub fn build_proposals(
    raw_candidates: Vec<serde_json::Value>,
    source_sha256: &str,
    source_text: &str,
) -> (Vec<AssistProposal>, Vec<String>) {
    let mut proposals = Vec::new();
    let mut warnings = Vec::new();
    for (i, raw) in raw_candidates.into_iter().enumerate() {
        match serde_json::from_value::<AssistCandidate>(raw) {
            Ok(candidate) => match validate_candidate(&candidate, source_sha256, source_text) {
                Ok(p) => proposals.push(p),
                Err(reason) => warnings.push(format!("candidate {i} rejected: {reason}")),
            },
            Err(e) => warnings.push(format!("candidate {i} rejected: malformed candidate ({e})")),
        }
    }
    (proposals, warnings)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE_TEXT: &str = "Section 4: First-Aid Measures\n\
        Inhalation: Remove to fresh air. Keep at rest.\n\
        Skin contact: Wash with plenty of soap and water.\n\
        Eye contact: Rinse cautiously with water for several minutes.\n\
        Ingestion: Rinse mouth. Do not induce vomiting.";
    const SOURCE_SHA: &str = "deadbeef";

    fn candidate(path: &str, value: &str, excerpt: &str, page: Option<u32>) -> AssistCandidate {
        AssistCandidate {
            path: path.to_string(),
            proposed_value: serde_json::json!(value),
            source_page: page,
            source_excerpt: excerpt.to_string(),
            rationale: Some("quoted directly from Section 4".to_string()),
        }
    }

    #[test]
    fn section4_allowlist_accepts_known_paths() {
        for path in SECTION4_ALLOWED_PATHS {
            assert!(is_allowed_path(path), "{path} should be allowed");
        }
    }

    #[test]
    fn section4_allowlist_rejects_section2_and_section9_paths() {
        assert!(!is_allowed_path(
            "HazardIdentification.Classification.HealthEffect.AcuteToxicityOral"
        ));
        assert!(!is_allowed_path("PhysicalChemicalProperties.FlashPoint"));
    }

    #[test]
    fn valid_candidate_is_accepted_with_supplier_sds_semantics() {
        let c = candidate(
            "FirstAidMeasures.ExposureRoute.FirstAidInhalation.FullText",
            "Remove to fresh air. Keep at rest.",
            "Remove to fresh air. Keep at rest.",
            Some(1),
        );
        let p = validate_candidate(&c, SOURCE_SHA, SOURCE_TEXT).unwrap();
        assert_eq!(p.confidence, ConfidenceLevel::Medium);
        assert_eq!(p.path, c.path);
        assert!(p.id.starts_with("assist-"));
    }

    #[test]
    fn confidence_never_exceeds_medium() {
        // ASSIST_CONFIDENCE is a compile-time constant, not a runtime
        // choice -- this test pins that it can never silently become High.
        assert_eq!(ASSIST_CONFIDENCE, ConfidenceLevel::Medium);
        assert_ne!(ASSIST_CONFIDENCE, ConfidenceLevel::High);
    }

    #[test]
    fn rejects_unsupported_section2_path() {
        let c = candidate(
            "HazardIdentification.Classification.HealthEffect.AcuteToxicityOral",
            "Category 3",
            "Remove to fresh air.",
            None,
        );
        assert!(validate_candidate(&c, SOURCE_SHA, SOURCE_TEXT).is_err());
    }

    #[test]
    fn rejects_unsupported_section9_path() {
        let c = candidate(
            "PhysicalChemicalProperties.FlashPoint",
            "23 degC",
            "Remove to fresh air.",
            None,
        );
        assert!(validate_candidate(&c, SOURCE_SHA, SOURCE_TEXT).is_err());
    }

    #[test]
    fn rejects_empty_excerpt() {
        let c = candidate(
            "FirstAidMeasures.DescriptionOfFirstAidMeasures.FullText",
            "Remove to fresh air.",
            "",
            None,
        );
        assert!(validate_candidate(&c, SOURCE_SHA, SOURCE_TEXT).is_err());
    }

    #[test]
    fn rejects_excerpt_absent_from_source() {
        let c = candidate(
            "FirstAidMeasures.DescriptionOfFirstAidMeasures.FullText",
            "Administer oxygen immediately.",
            "Administer oxygen immediately.",
            None,
        );
        assert!(validate_candidate(&c, SOURCE_SHA, SOURCE_TEXT).is_err());
    }

    #[test]
    fn rejects_zero_source_page() {
        let c = candidate(
            "FirstAidMeasures.DescriptionOfFirstAidMeasures.FullText",
            "Remove to fresh air.",
            "Remove to fresh air.",
            Some(0),
        );
        assert!(validate_candidate(&c, SOURCE_SHA, SOURCE_TEXT).is_err());
    }

    #[test]
    fn rejects_nonstring_proposed_value() {
        let c = AssistCandidate {
            path: "FirstAidMeasures.DescriptionOfFirstAidMeasures.FullText".to_string(),
            proposed_value: serde_json::json!({"nested": "object"}),
            source_page: None,
            source_excerpt: "Remove to fresh air.".to_string(),
            rationale: None,
        };
        assert!(validate_candidate(&c, SOURCE_SHA, SOURCE_TEXT).is_err());
    }

    #[test]
    fn deterministic_ids_are_stable_across_reruns() {
        let c = candidate(
            "FirstAidMeasures.ExposureRoute.FirstAidInhalation.FullText",
            "Remove to fresh air. Keep at rest.",
            "Remove to fresh air. Keep at rest.",
            Some(1),
        );
        let p1 = validate_candidate(&c, SOURCE_SHA, SOURCE_TEXT).unwrap();
        let p2 = validate_candidate(&c, SOURCE_SHA, SOURCE_TEXT).unwrap();
        assert_eq!(p1.id, p2.id);
    }

    #[test]
    fn different_source_sha_changes_the_id() {
        let c = candidate(
            "FirstAidMeasures.ExposureRoute.FirstAidInhalation.FullText",
            "Remove to fresh air. Keep at rest.",
            "Remove to fresh air. Keep at rest.",
            Some(1),
        );
        let p1 = validate_candidate(&c, "sha-a", SOURCE_TEXT).unwrap();
        let p2 = validate_candidate(&c, "sha-b", SOURCE_TEXT).unwrap();
        assert_ne!(p1.id, p2.id);
    }

    #[test]
    fn model_supplied_id_and_confidence_fields_reject_the_candidate() {
        let raw = serde_json::json!({
            "id": "attacker-chosen-id",
            "path": "FirstAidMeasures.ExposureRoute.FirstAidInhalation.FullText",
            "proposed_value": "Remove to fresh air. Keep at rest.",
            "source_page": 1,
            "source_excerpt": "Remove to fresh air. Keep at rest.",
            "rationale": null,
        });
        assert!(serde_json::from_value::<AssistCandidate>(raw).is_err());

        let raw_confidence = serde_json::json!({
            "path": "FirstAidMeasures.ExposureRoute.FirstAidInhalation.FullText",
            "proposed_value": "Remove to fresh air. Keep at rest.",
            "source_page": 1,
            "source_excerpt": "Remove to fresh air. Keep at rest.",
            "rationale": null,
            "confidence": "high",
        });
        assert!(serde_json::from_value::<AssistCandidate>(raw_confidence).is_err());
    }

    #[test]
    fn build_proposals_omits_invalid_candidates_with_warnings_not_aborting_the_batch() {
        let raw_candidates = vec![
            serde_json::json!({
                "path": "FirstAidMeasures.ExposureRoute.FirstAidInhalation.FullText",
                "proposed_value": "Remove to fresh air. Keep at rest.",
                "source_page": 1,
                "source_excerpt": "Remove to fresh air. Keep at rest.",
                "rationale": null,
            }),
            serde_json::json!({
                "path": "PhysicalChemicalProperties.FlashPoint",
                "proposed_value": "23 degC",
                "source_page": 1,
                "source_excerpt": "Remove to fresh air.",
                "rationale": null,
            }),
            serde_json::json!({
                "path": "FirstAidMeasures.DescriptionOfFirstAidMeasures.FullText",
                "proposed_value": "Administer oxygen immediately.",
                "source_page": 1,
                "source_excerpt": "Administer oxygen immediately.",
                "rationale": null,
            }),
        ];
        let (proposals, warnings) = build_proposals(raw_candidates, SOURCE_SHA, SOURCE_TEXT);
        assert_eq!(proposals.len(), 1);
        assert_eq!(warnings.len(), 2);
        assert!(warnings[0].contains("candidate 1"));
        assert!(warnings[1].contains("candidate 2"));
    }

    #[test]
    fn parse_candidates_json_rejects_non_array_top_level() {
        assert!(parse_candidates_json("{}").is_err());
        assert!(parse_candidates_json("not json").is_err());
        assert!(parse_candidates_json("[]").unwrap().is_empty());
    }

    #[test]
    fn excerpt_verifies_across_reflowed_whitespace() {
        assert!(excerpt_verifies(
            "Section 7: Keep container\ntightly   closed.\nStore in a cool place.",
            "Keep container tightly closed."
        ));
    }

    #[test]
    fn excerpt_verifies_rejects_absent_text() {
        assert!(!excerpt_verifies(
            "Section 7: Store in a cool place.",
            "Keep container tightly closed."
        ));
    }
}
