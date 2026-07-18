use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::converter::validator::{validate_cas_format, Finding};
use crate::enrichment::{lookup_cas, CasInfo};
use crate::schema::SdsRoot;

use super::draft::{draft_sections_from_resolved_input, SectionDraftResult};
use super::input::ProductInput;
use super::provenance::{path, EvidenceLevel, FieldProvenance};
use super::unresolved::{
    build_lookup_failure_unresolved, build_product_level_unresolved, FieldStatus,
    NotApplicableReason, UnresolvedField,
};

/// Generation must never mark its own output approved — approval is a
/// separate human act (approver, timestamp, target version), never a side
/// effect of running the generator. `Draft`/`ReviewRequired`/`Blocked` are
/// the only values [`compute_release_status`] can produce; `Approved`
/// exists on the enum for a future explicit human-approval record to set,
/// not for generation code to reach (see `tests::generation_never_approves`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseStatus {
    Draft,
    ReviewRequired,
    Blocked,
    Approved,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseGateResult {
    pub status: ReleaseStatus,
    pub blocking_findings: Vec<Finding>,
    pub required_actions: Vec<String>,
}

/// Deterministic tally over a [`GenerationResult`]'s provenance/unresolved
/// records — never manually incremented elsewhere, always recomputed by
/// [`compute_evidence_summary`] from the records themselves, so the counts
/// can't drift from what's actually in the report.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceSummary {
    pub confirmed: usize,
    pub supplied: usize,
    pub literature: usize,
    pub calculated: usize,
    pub estimated: usize,
    pub unresolved: usize,
    pub not_applicable: usize,
    pub product_test_evidence: usize,
    pub unverified_user_input: usize,
}

/// The full generation report: the official SDS draft plus everything
/// needed to understand its incompleteness. `sds` is the only part that
/// belongs in `official_sds.json` — `findings`/`unresolved`/`provenance`/
/// `evidence_summary`/`release_status` describe the draft, they are never
/// written into it (see `tests::official_sds_json_has_no_report_keys`).
///
/// Uses `SdsRoot` directly, not an aspirational `DomainSds` type — the
/// regulatory-profile/domain separation described in
/// `docs/sdsforge-architecture.md` doesn't exist yet, and commit #9
/// deliberately returned the current MHLW-backed `SdsRoot`. This is that
/// same decision carried forward: `sds: SdsRoot` is today's `mhlw-v1`
/// representation, not a placeholder for a schema refactor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationResult {
    pub sds: SdsRoot,
    pub findings: Vec<Finding>,
    pub unresolved: Vec<UnresolvedField>,
    pub provenance: Vec<FieldProvenance>,
    pub evidence_summary: EvidenceSummary,
    pub release_status: ReleaseStatus,
}

/// Pure, deterministic: wraps [`draft_sections_from_resolved_input`] (does
/// not duplicate its mapping logic) and adds provenance/unresolved/
/// evidence-summary/release-status on top. Same `resolved` shape as commit
/// #9's function — a `HashMap` of successful CAS lookups only, so a
/// missing entry cannot be distinguished as "not found" vs. "request
/// failed" here either; see [`build_lookup_failure_unresolved`]'s doc
/// comment for why that's a recorded limitation, not a redesign target for
/// this commit.
pub fn generate_from_resolved_input(
    input: &ProductInput,
    resolved: &HashMap<String, CasInfo>,
) -> GenerationResult {
    let SectionDraftResult { sds, findings } = draft_sections_from_resolved_input(input, resolved);

    let mut provenance = Vec::new();
    let mut unresolved = Vec::new();

    provenance.push(FieldProvenance::supplied(
        path::TRADE_NAME_JP,
        "supplied in ProductInput",
    ));
    if !input.other_names.is_empty() {
        provenance.push(FieldProvenance::supplied(
            path::OTHER_NAME,
            "supplied in ProductInput",
        ));
    }
    provenance.push(FieldProvenance::supplied(
        path::SUPPLIER_COMPANY_NAME,
        "supplied in ProductInput",
    ));
    if input.supplier.address.is_some() {
        provenance.push(FieldProvenance::supplied(
            path::SUPPLIER_ADDRESS,
            "supplied in ProductInput",
        ));
    }
    if input.supplier.phone.is_some() {
        provenance.push(FieldProvenance::supplied(
            path::SUPPLIER_PHONE,
            "supplied in ProductInput",
        ));
    }
    if input.supplier.email.is_some() {
        provenance.push(FieldProvenance::supplied(
            path::SUPPLIER_EMAIL,
            "supplied in ProductInput",
        ));
    }

    for (i, component) in input.components.iter().enumerate() {
        if component.name.is_some() {
            provenance.push(FieldProvenance::supplied(
                path::composition_row(i, path::GENERIC_NAME),
                "supplied in ComponentInput",
            ));
        }

        if let Some(cas) = &component.cas_number {
            provenance.push(FieldProvenance::supplied(
                path::composition_row(i, path::CAS_NO),
                "supplied and check-digit validated",
            ));

            match resolved.get(cas) {
                Some(info) => {
                    if info.iupac_name.is_some() {
                        provenance.push(FieldProvenance::from_cas_resolver(
                            path::composition_row(i, path::IUPAC_NAME),
                            info.pubchem_cid,
                        ));
                    }
                    if info.molecular_formula.is_some() {
                        provenance.push(FieldProvenance::from_cas_resolver(
                            path::composition_row(i, path::MOLECULAR_FORMULA),
                            info.pubchem_cid,
                        ));
                    }
                }
                None => {
                    // Mirrors commit #9's own condition for emitting
                    // GEN-CAS-ENRICHMENT-MISSING: only for well-formed CAS
                    // numbers. A malformed one is already fully covered by
                    // commit #8's GEN-CAS-FORMAT finding — adding an
                    // unresolved-identity record on top would be redundant
                    // noise about the same underlying problem.
                    if validate_cas_format(cas) {
                        unresolved.push(build_lookup_failure_unresolved(
                            i,
                            cas,
                            component.name.as_deref(),
                        ));
                    }
                }
            }
        }

        // ConcentrationRange is always present on ComponentInput (not
        // Option), so this provenance record is unconditional.
        provenance.push(FieldProvenance::supplied(
            path::composition_row(i, path::CONCENTRATION),
            "supplied and structurally validated",
        ));
    }

    unresolved.extend(build_product_level_unresolved());

    let evidence_summary = compute_evidence_summary(&provenance, &unresolved);
    let release_status = compute_release_status(&unresolved, &findings);

    GenerationResult {
        sds,
        findings,
        unresolved,
        provenance,
        evidence_summary,
        release_status,
    }
}

/// Orchestration: performs the CAS lookups (reusing
/// [`crate::enrichment::lookup_cas`], no second PubChem client), then
/// delegates everything else to [`generate_from_resolved_input`]. This is
/// the one new orchestration path this commit adds — it does not call
/// commit #9's `generate_section_1_and_3`, which would re-fetch the same
/// CAS data over the network a second time for no benefit.
pub async fn generate_with_enrichment(
    input: &ProductInput,
    client: &reqwest::Client,
) -> GenerationResult {
    let mut resolved = HashMap::new();
    for component in &input.components {
        if let Some(cas) = &component.cas_number {
            if let Ok(Some(info)) = lookup_cas(cas, client).await {
                resolved.insert(cas.clone(), info);
            }
        }
    }
    generate_from_resolved_input(input, &resolved)
}

/// Maps an [`EvidenceLevel`] to the [`FieldStatus`] bucket it represents,
/// for evidence-summary tallying only — `FieldStatus`'s payload is `()`
/// here since the summary only needs counts, not the underlying values.
fn evidence_level_bucket(level: &EvidenceLevel) -> FieldStatus<()> {
    match level {
        EvidenceLevel::ProductTestReport | EvidenceLevel::EquivalentBatchTestReport => {
            FieldStatus::Confirmed(())
        }
        EvidenceLevel::SupplierSpecification
        | EvidenceLevel::SupplierSds
        | EvidenceLevel::UnverifiedUserInput => FieldStatus::Supplied(()),
        EvidenceLevel::RegulatoryDatabase
        | EvidenceLevel::PeerReviewedLiterature
        | EvidenceLevel::ReferenceDatabase => FieldStatus::Literature(()),
        EvidenceLevel::DeterministicCalculation => FieldStatus::Calculated(()),
        EvidenceLevel::ModelEstimate => FieldStatus::Estimated(()),
        EvidenceLevel::None => FieldStatus::NotApplicable(NotApplicableReason {
            explanation: "no evidence source recorded".into(),
        }),
    }
}

/// Recomputes the summary from scratch every time — see
/// [`EvidenceSummary`]'s doc comment for why this is deliberate.
pub fn compute_evidence_summary(
    provenance: &[FieldProvenance],
    unresolved: &[UnresolvedField],
) -> EvidenceSummary {
    let mut summary = EvidenceSummary {
        unresolved: unresolved.len(),
        ..Default::default()
    };

    for p in provenance {
        match evidence_level_bucket(&p.source_type) {
            FieldStatus::Confirmed(()) => summary.confirmed += 1,
            FieldStatus::Supplied(()) => summary.supplied += 1,
            FieldStatus::Literature(()) => summary.literature += 1,
            FieldStatus::Calculated(()) => summary.calculated += 1,
            FieldStatus::Estimated(()) => summary.estimated += 1,
            FieldStatus::NotApplicable(_) => summary.not_applicable += 1,
            FieldStatus::Unresolved(_) => {} // unreachable: evidence_level_bucket never returns this variant
        }
        if matches!(
            p.source_type,
            EvidenceLevel::ProductTestReport | EvidenceLevel::EquivalentBatchTestReport
        ) {
            summary.product_test_evidence += 1;
        }
        if p.source_type == EvidenceLevel::UnverifiedUserInput {
            summary.unverified_user_input += 1;
        }
    }

    summary
}

/// `Blocked` if anything explicitly blocks release (an unresolved field
/// marked `blocks_release`, or a CRIT/HIGH validation finding); otherwise
/// `ReviewRequired` if anything at all remains unresolved or was merely
/// flagged; `Draft` only when there's truly nothing to review. Given this
/// feature only ever produces Section 1/3 and always adds the seven
/// product-level unresolved fields, real results are `Blocked` or
/// `ReviewRequired` — `Draft` is reachable but not the common case, exactly
/// as intended: an incomplete draft should not look deceptively finished.
///
/// Never returns `Approved` — there is no branch that produces it.
pub fn compute_release_status(
    unresolved: &[UnresolvedField],
    findings: &[Finding],
) -> ReleaseStatus {
    let blocked_by_unresolved = unresolved.iter().any(|f| f.blocks_release);
    let blocked_by_findings = findings
        .iter()
        .any(|f| f.level == "CRIT" || f.level == "HIGH");
    if blocked_by_unresolved || blocked_by_findings {
        return ReleaseStatus::Blocked;
    }
    if !unresolved.is_empty() || !findings.is_empty() {
        return ReleaseStatus::ReviewRequired;
    }
    ReleaseStatus::Draft
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generation::input::{ComponentInput, ConcentrationRange, SupplierInput};

    fn supplier() -> SupplierInput {
        SupplierInput {
            company_name: "Example Chemical Co.".into(),
            address: Some("1-1 Example, Tokyo".into()),
            phone: Some("03-1234-5678".into()),
            email: Some("safety@example.com".into()),
        }
    }

    fn exact_component(cas: &str, name: &str, value: f64) -> ComponentInput {
        ComponentInput {
            cas_number: Some(cas.into()),
            name: Some(name.into()),
            concentration: ConcentrationRange {
                exact: Some(value),
                lower: None,
                upper: None,
                unit: "%".into(),
            },
        }
    }

    fn product() -> ProductInput {
        ProductInput {
            trade_name: "Test Solvent".into(),
            other_names: vec![],
            supplier: supplier(),
            components: vec![exact_component("7732-18-5", "Water", 100.0)],
        }
    }

    #[test]
    fn every_populated_section1_field_has_provenance() {
        let result = generate_from_resolved_input(&product(), &HashMap::new());
        let paths: Vec<&str> = result.provenance.iter().map(|p| p.path.as_str()).collect();
        assert!(paths.contains(&path::TRADE_NAME_JP));
        assert!(paths.contains(&path::SUPPLIER_COMPANY_NAME));
        assert!(paths.contains(&path::SUPPLIER_ADDRESS));
        assert!(paths.contains(&path::SUPPLIER_PHONE));
        assert!(paths.contains(&path::SUPPLIER_EMAIL));
    }

    #[test]
    fn every_populated_section3_field_has_provenance() {
        let result = generate_from_resolved_input(&product(), &HashMap::new());
        let paths: Vec<&str> = result.provenance.iter().map(|p| p.path.as_str()).collect();
        assert!(paths.contains(&path::composition_row(0, path::GENERIC_NAME).as_str()));
        assert!(paths.contains(&path::composition_row(0, path::CAS_NO).as_str()));
        assert!(paths.contains(&path::composition_row(0, path::CONCENTRATION).as_str()));
    }

    #[test]
    fn user_input_provenance_is_never_confirmed() {
        let result = generate_from_resolved_input(&product(), &HashMap::new());
        assert!(result
            .provenance
            .iter()
            .filter(|p| p.method.contains("supplied"))
            .all(|p| p.source_type == EvidenceLevel::UnverifiedUserInput));
    }

    #[test]
    fn resolver_output_is_reference_database_not_confirmed() {
        let mut resolved = HashMap::new();
        resolved.insert(
            "7732-18-5".to_string(),
            CasInfo {
                cas: "7732-18-5".into(),
                iupac_name: Some("oxidane".into()),
                molecular_formula: Some("H2O".into()),
                pubchem_cid: Some(962),
            },
        );
        let result = generate_from_resolved_input(&product(), &resolved);
        let cas_provenance: Vec<_> = result
            .provenance
            .iter()
            .filter(|p| p.path.contains("IupacName") || p.path.contains("MolecularFormula"))
            .collect();
        assert_eq!(cas_provenance.len(), 2);
        assert!(cas_provenance
            .iter()
            .all(|p| p.source_type == EvidenceLevel::ReferenceDatabase));
    }

    #[test]
    fn lookup_failure_creates_finding_and_unresolved_identity() {
        let result = generate_from_resolved_input(&product(), &HashMap::new());
        assert!(result
            .findings
            .iter()
            .any(|f| f.rule == "GEN-CAS-ENRICHMENT-MISSING"));
        assert!(result
            .unresolved
            .iter()
            .any(|u| u.path == path::composition_row(0, path::CAS_NO)));
    }

    #[test]
    fn valid_supplied_composition_remains_after_lookup_failure() {
        let result = generate_from_resolved_input(&product(), &HashMap::new());
        let row = &result
            .sds
            .composition
            .as_ref()
            .unwrap()
            .composition_and_concentration
            .as_ref()
            .unwrap()[0];
        assert_eq!(
            row.substance_identifiers
                .as_ref()
                .unwrap()
                .substance_names
                .as_ref()
                .unwrap()
                .generic_name
                .as_deref(),
            Some("Water")
        );
    }

    #[test]
    fn evidence_summary_matches_underlying_records() {
        let result = generate_from_resolved_input(&product(), &HashMap::new());
        let expected_unverified = result
            .provenance
            .iter()
            .filter(|p| p.source_type == EvidenceLevel::UnverifiedUserInput)
            .count();
        assert_eq!(
            result.evidence_summary.unverified_user_input,
            expected_unverified
        );
        assert_eq!(result.evidence_summary.unresolved, result.unresolved.len());
        assert_eq!(
            result.evidence_summary.supplied
                + result.evidence_summary.confirmed
                + result.evidence_summary.literature
                + result.evidence_summary.calculated
                + result.evidence_summary.estimated
                + result.evidence_summary.not_applicable,
            result.provenance.len()
        );
    }

    #[test]
    fn blocking_unresolved_produces_blocked() {
        let unresolved = vec![UnresolvedField {
            path: "x".into(),
            title: "x".into(),
            reason: super::super::unresolved::UnresolvedReason::MissingInput,
            required_inputs: vec![],
            acceptable_evidence: vec![],
            safety_impact: super::super::unresolved::SafetyImpact::High,
            regulatory_impact: super::super::unresolved::RegulatoryImpact::High,
            recommended_action: "x".into(),
            blocks_release: true,
        }];
        assert_eq!(
            compute_release_status(&unresolved, &[]),
            ReleaseStatus::Blocked
        );
    }

    #[test]
    fn nonblocking_unresolved_produces_review_required() {
        let result = generate_from_resolved_input(&product(), &HashMap::new());
        // The product-level unresolved fields are all non-blocking, so a
        // typical result is ReviewRequired, not Blocked or a falsely-clean Draft.
        assert_eq!(result.release_status, ReleaseStatus::ReviewRequired);
    }

    #[test]
    fn high_severity_finding_produces_blocked() {
        let finding = Finding {
            level: "HIGH".into(),
            rule: "X".into(),
            message: "x".into(),
        };
        assert_eq!(
            compute_release_status(&[], std::slice::from_ref(&finding)),
            ReleaseStatus::Blocked
        );
    }

    #[test]
    fn empty_input_with_no_issues_produces_draft() {
        assert_eq!(compute_release_status(&[], &[]), ReleaseStatus::Draft);
    }

    #[test]
    fn generation_never_approves() {
        // Structural guarantee: compute_release_status has no branch that
        // returns Approved. Exercised across several scenarios to confirm
        // empirically as well.
        let scenarios: Vec<(Vec<UnresolvedField>, Vec<Finding>)> = vec![
            (vec![], vec![]),
            (build_product_level_unresolved(), vec![]),
            (
                vec![],
                vec![Finding {
                    level: "CRIT".into(),
                    rule: "X".into(),
                    message: "x".into(),
                }],
            ),
        ];
        for (unresolved, findings) in scenarios {
            assert_ne!(
                compute_release_status(&unresolved, &findings),
                ReleaseStatus::Approved
            );
        }
        // And the real generation path, end to end:
        assert_ne!(
            generate_from_resolved_input(&product(), &HashMap::new()).release_status,
            ReleaseStatus::Approved
        );
    }

    #[test]
    fn official_sds_json_has_no_report_keys() {
        let result = generate_from_resolved_input(&product(), &HashMap::new());
        let sds_json = serde_json::to_value(&result.sds).unwrap();
        let as_object = sds_json.as_object().unwrap();
        for key in [
            "findings",
            "provenance",
            "unresolved",
            "evidence_summary",
            "release_status",
        ] {
            assert!(!as_object.contains_key(key));
        }
    }

    #[test]
    fn report_json_has_stable_snake_case_enum_values() {
        let result = generate_from_resolved_input(&product(), &HashMap::new());
        let report_json = serde_json::to_value(&result).unwrap();
        let release_status = report_json["release_status"].as_str().unwrap();
        assert_eq!(release_status, "review_required");
        // First unresolved entry is the lookup-failure record (pushed
        // before the always-present product-level ones).
        let first_unresolved_reason = report_json["unresolved"][0]["reason"].as_str().unwrap();
        assert_eq!(first_unresolved_reason, "missing_input");
        let reasons: Vec<&str> = report_json["unresolved"]
            .as_array()
            .unwrap()
            .iter()
            .map(|u| u["reason"].as_str().unwrap())
            .collect();
        assert!(reasons.contains(&"human_review_required"));
    }

    #[test]
    fn commit8_findings_are_retained_alongside_commit9_findings() {
        use crate::generation::validate_product_input;
        let mut p = product();
        p.components
            .push(exact_component("7732-18-5", "Water again", 0.0));
        let input_findings = validate_product_input(&p);
        assert!(input_findings.iter().any(|f| f.rule == "GEN-CAS-DUPLICATE"));
        // generate_from_resolved_input doesn't re-run validate_product_input
        // (that's the caller's job, same as commit #9) — confirm its own
        // findings (draft-mapping-time findings) coexist independently.
        let result = generate_from_resolved_input(&p, &HashMap::new());
        assert_eq!(
            result
                .sds
                .composition
                .as_ref()
                .unwrap()
                .composition_and_concentration
                .as_ref()
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn component_order_and_mapping_unchanged_from_commit9() {
        let mut p = product();
        p.components
            .push(exact_component("64-17-5", "Ethanol", 0.0));
        let result = generate_from_resolved_input(&p, &HashMap::new());
        let rows = result
            .sds
            .composition
            .as_ref()
            .unwrap()
            .composition_and_concentration
            .as_ref()
            .unwrap();
        assert_eq!(
            rows[0]
                .substance_identifiers
                .as_ref()
                .unwrap()
                .substance_names
                .as_ref()
                .unwrap()
                .generic_name
                .as_deref(),
            Some("Water")
        );
        assert_eq!(
            rows[1]
                .substance_identifiers
                .as_ref()
                .unwrap()
                .substance_names
                .as_ref()
                .unwrap()
                .generic_name
                .as_deref(),
            Some("Ethanol")
        );
    }

    #[test]
    fn repeated_generation_is_byte_equivalent() {
        let p = product();
        let a = generate_from_resolved_input(&p, &HashMap::new());
        let b = generate_from_resolved_input(&p, &HashMap::new());
        let json_a = serde_json::to_string(&a).unwrap();
        let json_b = serde_json::to_string(&b).unwrap();
        assert_eq!(json_a, json_b);
    }
}
