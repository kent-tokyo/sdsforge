use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::converter::validator::{validate_cas_format, Finding};
use crate::enrichment::{lookup_cas, lookup_cas_detailed, CasInfo, CasResolution};
use crate::normalize::{ChemicalNormalizer, NormalizationIssue, NormalizationStatus};
use crate::schema::SdsRoot;

use super::draft::{draft_sections_from_resolved_input, SectionDraftResult};
use super::input::ProductInput;
use super::provenance::{path, EvidenceLevel, FieldProvenance};
use super::resolve;
use super::unresolved::{
    build_lookup_failure_unresolved, FieldStatus, NotApplicableReason, RegulatoryImpact,
    RequiredInput, SafetyImpact, UnresolvedField, UnresolvedReason,
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
    let SectionDraftResult { mut sds, findings } =
        draft_sections_from_resolved_input(input, resolved);

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

    // Resolves the seven safety-sensitive properties against supplied
    // measured-property evidence — writes into `sds` only on full policy
    // satisfaction, otherwise adds a (more specific than commit #10 could
    // give) UnresolvedField. See resolve.rs's module doc for the "no
    // partial credit" rule this enforces.
    let (property_unresolved, property_provenance) =
        resolve::resolve_measured_properties(input, &mut sds);
    unresolved.extend(property_unresolved);
    provenance.extend(property_provenance);

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

/// Adds chematic-backed chemical-identity normalization on top of
/// [`generate_from_resolved_input`] — reuses it rather than duplicating
/// Section 1/3 mapping logic. Strategy: derive a plain `HashMap<String,
/// CasInfo>` containing only genuinely `CasResolution::Resolved` (non-
/// ambiguous) candidates and call the existing, unchanged
/// `generate_from_resolved_input` — an ambiguous CAS is simply absent from
/// that map, so the base pass treats it exactly like today's "lookup
/// didn't resolve" case (commit #9's `GEN-CAS-ENRICHMENT-MISSING`/
/// `MissingInput`, unchanged). This function then does one additive pass
/// over the already-built result: replaces the generic lookup-failure
/// unresolved entry with a specific `AmbiguousChemicalIdentity` one for
/// ambiguous components, and for resolved components runs the normalizer
/// and layers on canonical-SMILES writing / formula-consistency handling.
///
/// Never touches any of the seven product-level properties commit A
/// (2ac2758/d4dd15d) governs — this function's writes are scoped entirely
/// to `Composition.CompositionAndConcentration[i].{MolecularFormula,SMILES}`.
pub fn generate_from_normalized_input<N: ChemicalNormalizer>(
    input: &ProductInput,
    resolved: &HashMap<String, CasResolution>,
    normalizer: &N,
) -> GenerationResult {
    let basic: HashMap<String, CasInfo> = resolved
        .iter()
        .filter_map(|(cas, res)| match res {
            CasResolution::Resolved(c) => Some((
                cas.clone(),
                CasInfo {
                    cas: c.cas.clone(),
                    iupac_name: c.iupac_name.clone(),
                    molecular_formula: c.molecular_formula.clone(),
                    pubchem_cid: c.pubchem_cid,
                },
            )),
            _ => None,
        })
        .collect();

    let mut result = generate_from_resolved_input(input, &basic);

    for (i, component) in input.components.iter().enumerate() {
        let Some(cas) = &component.cas_number else {
            continue;
        };
        match resolved.get(cas) {
            Some(CasResolution::Ambiguous(candidates)) => {
                apply_ambiguous_identity(&mut result, i, cas, candidates);
            }
            Some(CasResolution::Resolved(candidate)) => {
                let normalization = normalizer.normalize(candidate);
                apply_normalization(&mut result, i, candidate, &normalization);
            }
            Some(CasResolution::NotFound) | None => {
                // Already handled by the base pass (GEN-CAS-ENRICHMENT-MISSING /
                // MissingInput) — nothing additional to do here.
            }
        }
    }

    result.evidence_summary = compute_evidence_summary(&result.provenance, &result.unresolved);
    result.release_status = compute_release_status(&result.unresolved, &result.findings);
    result
}

/// Orchestration for the `sdsforge generate --enrich` CLI path: resolves
/// every distinct CAS number in `input` through [`lookup_cas_detailed`]
/// (deduplicated — one request per distinct CAS, not per component), then
/// delegates everything else to [`generate_from_detailed_lookups`]. A
/// network/HTTP/parse failure for one CAS never aborts the whole draft —
/// it's recorded as a `Result::Err` and surfaces as a
/// `GEN-CAS-LOOKUP-ERROR` finding, while every other component's lookup
/// still proceeds normally.
pub async fn generate_with_detailed_enrichment<N: ChemicalNormalizer>(
    input: &ProductInput,
    client: &reqwest::Client,
    normalizer: &N,
) -> GenerationResult {
    let mut cas_numbers: Vec<String> = input
        .components
        .iter()
        .filter_map(|c| c.cas_number.clone())
        .collect();
    cas_numbers.sort();
    cas_numbers.dedup();

    let mut lookups: HashMap<String, Result<CasResolution, String>> = HashMap::new();
    for cas in cas_numbers {
        let outcome = lookup_cas_detailed(&cas, client)
            .await
            .map_err(|e| e.to_string());
        lookups.insert(cas, outcome);
    }

    generate_from_detailed_lookups(input, &lookups, normalizer)
}

/// Pure core of [`generate_with_detailed_enrichment`] — no network access,
/// so it's fully unit-testable with a fixture `lookups` map (see
/// `tests::lookup_error_is_nonblocking_and_retains_supplied_data`).
///
/// Delegates Section 1/3 mapping, ambiguity handling, and normalization
/// entirely to [`generate_from_normalized_input`] — every generation path
/// converges on that one implementation, this function does not duplicate
/// it. A CAS whose lookup failed (`Err`) is simply absent from the
/// `CasResolution` map passed down, so the base pass already creates its
/// usual "identity not resolved" [`UnresolvedField`] for it (same as a
/// `NotFound`/never-looked-up CAS) — this function's only additional job
/// is layering a `GEN-CAS-LOOKUP-ERROR` finding on top, which is the one
/// thing that distinguishes "the request itself failed" from "nothing was
/// ever tried" (see [`build_lookup_failure_unresolved`]'s doc comment).
/// Never classified as `AmbiguousChemicalIdentity` — that reason is
/// reserved for a genuine multi-candidate resolver response — and never
/// promoted above MED severity, so a transient network failure alone
/// cannot block a release the way ambiguity or a formula mismatch does.
pub fn generate_from_detailed_lookups<N: ChemicalNormalizer>(
    input: &ProductInput,
    lookups: &HashMap<String, Result<CasResolution, String>>,
    normalizer: &N,
) -> GenerationResult {
    let resolved: HashMap<String, CasResolution> = lookups
        .iter()
        .filter_map(|(cas, outcome)| match outcome {
            Ok(resolution) => Some((cas.clone(), resolution.clone())),
            Err(_) => None,
        })
        .collect();

    let mut result = generate_from_normalized_input(input, &resolved, normalizer);

    for (i, component) in input.components.iter().enumerate() {
        let Some(cas) = &component.cas_number else {
            continue;
        };
        // Mirrors generate_from_resolved_input's own guard: a malformed CAS
        // already has its own GEN-CAS-FORMAT finding from input validation,
        // so a lookup-error finding on top would be redundant noise about
        // the same underlying problem.
        if let Some(Err(message)) = lookups.get(cas) {
            if validate_cas_format(cas) {
                result.findings.push(Finding {
                    level: "MED".into(),
                    rule: "GEN-CAS-LOOKUP-ERROR".into(),
                    message: format!(
                        "CAS lookup failed for component {} (CAS {cas}): {message}. Supplied \
                         identity and composition data was retained; provide an authoritative \
                         identity source or retry enrichment.",
                        i + 1
                    ),
                });
            }
        }
    }

    result.evidence_summary = compute_evidence_summary(&result.provenance, &result.unresolved);
    result.release_status = compute_release_status(&result.unresolved, &result.findings);
    result
}

fn apply_ambiguous_identity(
    result: &mut GenerationResult,
    component_index: usize,
    cas: &str,
    candidates: &[crate::enrichment::ChemicalIdentityCandidate],
) {
    let cas_path = path::composition_row(component_index, path::CAS_NO);
    let cids: Vec<String> = candidates
        .iter()
        .filter_map(|c| c.pubchem_cid)
        .map(|cid| cid.to_string())
        .collect();

    result.findings.push(Finding {
        level: "HIGH".into(),
        rule: "GEN-CAS-AMBIGUOUS".into(),
        message: format!(
            "CAS '{cas}': PubChem returned {} distinct candidates (CIDs: {}) — a material identity \
             ambiguity, not resolved automatically.",
            candidates.len(),
            if cids.is_empty() { "unknown".to_string() } else { cids.join(", ") }
        ),
    });

    // Replace the generic lookup-failure entry the base pass created (the
    // CAS wasn't in the derived CasInfo map, so it looks like a plain
    // "not found" to generate_from_resolved_input) with a more specific one.
    result.unresolved.retain(|f| f.path != cas_path);
    result.unresolved.push(UnresolvedField {
        path: cas_path,
        title: format!("Ambiguous chemical identity for CAS '{cas}'"),
        reason: UnresolvedReason::AmbiguousChemicalIdentity,
        required_inputs: vec![RequiredInput::new(
            "authoritative_identity_source",
            format!(
                "An authoritative supplier or regulatory source selecting one specific candidate \
                 identity from {} PubChem matches (CIDs: {}).",
                candidates.len(),
                if cids.is_empty() {
                    "unknown".to_string()
                } else {
                    cids.join(", ")
                }
            ),
        )],
        acceptable_evidence: vec![
            EvidenceLevel::SupplierSpecification,
            EvidenceLevel::SupplierSds,
            EvidenceLevel::RegulatoryDatabase,
        ],
        safety_impact: SafetyImpact::High,
        regulatory_impact: RegulatoryImpact::High,
        recommended_action:
            "Do not select a candidate by name similarity, CID order, or structure \
            size — obtain authoritative confirmation of which candidate matches this CAS number."
                .into(),
        blocks_release: true,
    });
}

fn apply_normalization(
    result: &mut GenerationResult,
    component_index: usize,
    candidate: &crate::enrichment::ChemicalIdentityCandidate,
    normalization: &crate::normalize::ChemicalNormalizationResult,
) {
    match normalization.status {
        NormalizationStatus::MissingStructure => {
            result.findings.push(Finding {
                level: "LOW".into(),
                rule: "GEN-STRUCTURE-MISSING".into(),
                message: format!(
                    "CAS '{}': resolver returned no SMILES structure — CAS/name/concentration remain usable.",
                    candidate.cas
                ),
            });
        }
        NormalizationStatus::InvalidStructure => {
            result.findings.push(Finding {
                level: "MED".into(),
                rule: "GEN-STRUCTURE-INVALID".into(),
                message: format!(
                    "CAS '{}': resolver-supplied SMILES could not be parsed — identity remains \
                     independently verifiable via CID/IUPAC name.",
                    candidate.cas
                ),
            });
        }
        NormalizationStatus::Ambiguous => {
            // Not reachable via this code path today (ambiguity is handled
            // at the CasResolution level before a normalizer ever runs),
            // kept for completeness of the match.
        }
        NormalizationStatus::Normalized | NormalizationStatus::ReviewRequired => {
            let has_mismatch = normalization
                .issues
                .contains(&NormalizationIssue::FormulaMismatch);
            let has_multi_fragment = normalization
                .issues
                .contains(&NormalizationIssue::MultiFragmentStructure);

            if has_mismatch {
                let calculated = normalization
                    .calculated
                    .molecular_formula
                    .as_deref()
                    .unwrap_or("?");
                let resolver_formula = candidate.molecular_formula.as_deref().unwrap_or("?");
                result.findings.push(Finding {
                    level: "HIGH".into(),
                    rule: "GEN-STRUCTURE-FORMULA-MISMATCH".into(),
                    message: format!(
                        "CAS '{}': resolver formula '{resolver_formula}' does not match chematic-calculated \
                         formula '{calculated}' from the resolved structure — not reconciled automatically.",
                        candidate.cas
                    ),
                });
                // Don't expose two conflicting values in the official field
                // — remove whatever the base pass already wrote.
                remove_molecular_formula(result, component_index);
            }
            if has_multi_fragment {
                result.findings.push(Finding {
                    level: "MED".into(),
                    rule: "GEN-STRUCTURE-MULTIFRAGMENT".into(),
                    message: format!(
                        "CAS '{}': structure has more than one disconnected fragment (e.g. a salt or \
                         solvate) — reported for review, not automatically rejected or reduced to its \
                         largest fragment.",
                        candidate.cas
                    ),
                });
            }

            // Canonical SMILES is written whenever normalization produced
            // one at all -- including the multi-fragment/formula-mismatch
            // review cases, since the structure itself parsed successfully
            // and the canonical form is still the accurate representation
            // of what was parsed.
            if let Some(canonical) = &normalization.canonical_smiles {
                set_smiles(result, component_index, canonical);
                // PubChem's full `smiles` is preferred as the normalization
                // input; `connectivity_smiles` (no stereochemistry/isotopes)
                // is used only as a fallback -- flag it here so the report
                // discloses when that weaker representation was the source.
                let used_connectivity_fallback =
                    candidate.smiles.is_none() && candidate.connectivity_smiles.is_some();
                result.provenance.push(FieldProvenance::source_smiles(
                    candidate.pubchem_cid,
                    used_connectivity_fallback,
                ));
                result.provenance.push(FieldProvenance::canonical_smiles(
                    candidate.pubchem_cid,
                    normalization
                        .issues
                        .iter()
                        .map(|i| format!("{i:?}"))
                        .collect(),
                ));
            }
        }
    }
}

fn composition_row_mut(
    result: &mut GenerationResult,
    index: usize,
) -> Option<&mut crate::schema::CompositionCompositionAndConcentration> {
    result
        .sds
        .composition
        .as_mut()?
        .composition_and_concentration
        .as_mut()?
        .get_mut(index)
}

fn set_smiles(result: &mut GenerationResult, index: usize, canonical: &str) {
    if let Some(row) = composition_row_mut(result, index) {
        row.smiles = Some(canonical.to_string());
    }
}

fn remove_molecular_formula(result: &mut GenerationResult, index: usize) {
    if let Some(row) = composition_row_mut(result, index) {
        row.molecular_formula = None;
    }
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

/// Aggregates a [`GenerationResult`] into the CRIT/HIGH findings and
/// required actions that actually block release — deterministic, built
/// from `result.unresolved`/`result.findings` only, never a second source
/// of truth. `required_actions` is deduplicated by exact string match
/// (sufficient for now — multiple unresolved fields commonly share the
/// same recommended action, e.g. two properties both needing "a human
/// must first determine whether this property applies").
pub fn evaluate_release_gate(result: &GenerationResult) -> ReleaseGateResult {
    let blocking_findings: Vec<Finding> = result
        .findings
        .iter()
        .filter(|f| f.level == "CRIT" || f.level == "HIGH")
        .cloned()
        .collect();

    let mut required_actions = Vec::new();
    for field in result.unresolved.iter().filter(|f| f.blocks_release) {
        if !required_actions.contains(&field.recommended_action) {
            required_actions.push(field.recommended_action.clone());
        }
    }

    ReleaseGateResult {
        status: result.release_status,
        blocking_findings,
        required_actions,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generation::input::{ComponentInput, ConcentrationRange, SupplierInput};
    use crate::generation::unresolved::build_product_level_unresolved;
    use crate::generation::{
        EvidenceApplicability, EvidenceSource, MeasuredValueEvidence, MeasurementConditions,
    };

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
            measured_properties: Default::default(),
            evidence: vec![],
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

    fn product_with_confirmed_flash_point() -> ProductInput {
        let mut p = product();
        p.evidence.push(EvidenceSource {
            id: "ev1".into(),
            level: EvidenceLevel::ProductTestReport,
            reference: "Lab Report 2026-014".into(),
            issuer: None,
            document_date: None,
            applies_to: EvidenceApplicability::FinishedProduct,
        });
        p.measured_properties
            .flash_point
            .push(MeasuredValueEvidence {
                value: 61.0,
                unit: "°C".into(),
                method: Some("Closed Cup (ASTM D93)".into()),
                conditions: MeasurementConditions {
                    temperature_c: None,
                    pressure_kpa: None,
                    atmosphere: None,
                },
                sample_id: None,
                batch_id: None,
                evidence_id: "ev1".into(),
            });
        p
    }

    #[test]
    fn evidence_summary_counts_update_when_a_property_resolves() {
        let baseline = generate_from_resolved_input(&product(), &HashMap::new());
        let resolved =
            generate_from_resolved_input(&product_with_confirmed_flash_point(), &HashMap::new());

        assert_eq!(baseline.evidence_summary.confirmed, 0);
        assert_eq!(resolved.evidence_summary.confirmed, 1);
        // One fewer unresolved entry (flash point moved out of the list).
        assert_eq!(resolved.unresolved.len(), baseline.unresolved.len() - 1);
        assert_eq!(
            resolved.evidence_summary.unresolved,
            resolved.unresolved.len()
        );
    }

    #[test]
    fn resolving_a_property_updates_release_status_deterministically() {
        // Two disagreeing reports -> ConflictingSources -> Blocked.
        let mut blocked = product();
        blocked.evidence.push(EvidenceSource {
            id: "ev1".into(),
            level: EvidenceLevel::ProductTestReport,
            reference: "Report A".into(),
            issuer: None,
            document_date: None,
            applies_to: EvidenceApplicability::FinishedProduct,
        });
        blocked.evidence.push(EvidenceSource {
            id: "ev2".into(),
            level: EvidenceLevel::ProductTestReport,
            reference: "Report B".into(),
            issuer: None,
            document_date: None,
            applies_to: EvidenceApplicability::FinishedProduct,
        });
        let conds = MeasurementConditions {
            temperature_c: None,
            pressure_kpa: None,
            atmosphere: None,
        };
        blocked
            .measured_properties
            .flash_point
            .push(MeasuredValueEvidence {
                value: 61.0,
                unit: "°C".into(),
                method: Some("Closed Cup".into()),
                conditions: conds.clone(),
                sample_id: None,
                batch_id: None,
                evidence_id: "ev1".into(),
            });
        blocked
            .measured_properties
            .flash_point
            .push(MeasuredValueEvidence {
                value: 65.0,
                unit: "°C".into(),
                method: Some("Closed Cup".into()),
                conditions: conds,
                sample_id: None,
                batch_id: None,
                evidence_id: "ev2".into(),
            });
        let blocked_result = generate_from_resolved_input(&blocked, &HashMap::new());
        assert_eq!(blocked_result.release_status, ReleaseStatus::Blocked);

        // Same product, single agreeing report -> resolves cleanly, no
        // longer blocked by that property (still ReviewRequired overall —
        // six other product-level properties remain unresolved).
        let resolved_result =
            generate_from_resolved_input(&product_with_confirmed_flash_point(), &HashMap::new());
        assert_eq!(
            resolved_result.release_status,
            ReleaseStatus::ReviewRequired
        );
        assert!(!resolved_result
            .unresolved
            .iter()
            .any(|f| f.path.contains("FlashPoint") && f.blocks_release));
    }

    #[test]
    fn generation_with_resolved_evidence_still_never_approves() {
        let result =
            generate_from_resolved_input(&product_with_confirmed_flash_point(), &HashMap::new());
        assert_ne!(result.release_status, ReleaseStatus::Approved);
    }

    #[test]
    fn repeated_generation_with_evidence_is_byte_equivalent() {
        let p = product_with_confirmed_flash_point();
        let a = generate_from_resolved_input(&p, &HashMap::new());
        let b = generate_from_resolved_input(&p, &HashMap::new());
        assert_eq!(
            serde_json::to_string(&a).unwrap(),
            serde_json::to_string(&b).unwrap()
        );
    }

    #[test]
    fn evaluate_release_gate_aggregates_blocking_findings_and_dedupes_actions() {
        let mut p = product();
        // Two properties left unresolved with the same generic
        // HumanReviewRequired recommended_action text should collapse to
        // one deduplicated required_actions entry; a CRIT finding should
        // land in blocking_findings.
        let result = generate_from_resolved_input(&p, &HashMap::new());
        let gate = evaluate_release_gate(&result);
        assert_eq!(gate.status, result.release_status);
        // All seven product-level HumanReviewRequired fields share
        // identical recommended_action text, none blocks_release, so with
        // no evidence supplied there should be zero required_actions here
        // (nothing in this baseline is blocks_release: true).
        assert!(gate.required_actions.is_empty());

        // Force a duplicate blocking action via a conflicting-evidence
        // scenario on two properties sharing the same conflict message.
        p.evidence.push(EvidenceSource {
            id: "ev1".into(),
            level: EvidenceLevel::ProductTestReport,
            reference: "A".into(),
            issuer: None,
            document_date: None,
            applies_to: EvidenceApplicability::FinishedProduct,
        });
        p.evidence.push(EvidenceSource {
            id: "ev2".into(),
            level: EvidenceLevel::ProductTestReport,
            reference: "B".into(),
            issuer: None,
            document_date: None,
            applies_to: EvidenceApplicability::FinishedProduct,
        });
        let conds = MeasurementConditions {
            temperature_c: None,
            pressure_kpa: None,
            atmosphere: None,
        };
        p.measured_properties
            .flash_point
            .push(MeasuredValueEvidence {
                value: 61.0,
                unit: "°C".into(),
                method: Some("Closed Cup".into()),
                conditions: conds.clone(),
                sample_id: None,
                batch_id: None,
                evidence_id: "ev1".into(),
            });
        p.measured_properties
            .flash_point
            .push(MeasuredValueEvidence {
                value: 65.0,
                unit: "°C".into(),
                method: Some("Closed Cup".into()),
                conditions: conds.clone(),
                sample_id: None,
                batch_id: None,
                evidence_id: "ev2".into(),
            });
        p.measured_properties
            .boiling_point
            .push(MeasuredValueEvidence {
                value: 100.0,
                unit: "°C".into(),
                method: Some("ASTM D1120".into()),
                conditions: conds.clone(),
                sample_id: None,
                batch_id: None,
                evidence_id: "ev1".into(),
            });
        p.measured_properties
            .boiling_point
            .push(MeasuredValueEvidence {
                value: 105.0,
                unit: "°C".into(),
                method: Some("ASTM D1120".into()),
                conditions: conds,
                sample_id: None,
                batch_id: None,
                evidence_id: "ev2".into(),
            });
        let result = generate_from_resolved_input(&p, &HashMap::new());
        let gate = evaluate_release_gate(&result);
        assert_eq!(gate.status, ReleaseStatus::Blocked);
        // Both conflicts share identical recommended_action text -> deduplicated to one entry.
        assert_eq!(gate.required_actions.len(), 1);
    }

    use crate::enrichment::ChemicalIdentityCandidate;
    use crate::normalize::UnavailableNormalizer;

    fn identity_candidate(
        cas: &str,
        cid: u64,
        smiles: Option<&str>,
        formula: Option<&str>,
    ) -> ChemicalIdentityCandidate {
        ChemicalIdentityCandidate {
            cas: cas.into(),
            pubchem_cid: Some(cid),
            iupac_name: Some("test compound".into()),
            molecular_formula: formula.map(str::to_string),
            smiles: smiles.map(str::to_string),
            connectivity_smiles: None,
            inchi_key: None,
        }
    }

    #[test]
    fn multiple_candidates_are_not_silently_reduced_to_first() {
        let mut resolved = HashMap::new();
        resolved.insert(
            "7732-18-5".to_string(),
            CasResolution::Ambiguous(vec![
                identity_candidate("7732-18-5", 1, Some("O"), Some("H2O")),
                identity_candidate("7732-18-5", 2, Some("[OH2]"), Some("H2O")),
            ]),
        );
        let result = generate_from_normalized_input(&product(), &resolved, &UnavailableNormalizer);

        assert!(result
            .findings
            .iter()
            .any(|f| f.rule == "GEN-CAS-AMBIGUOUS"));
        let unresolved = result
            .unresolved
            .iter()
            .find(|f| f.path == path::composition_row(0, path::CAS_NO))
            .unwrap();
        assert_eq!(
            unresolved.reason,
            UnresolvedReason::AmbiguousChemicalIdentity
        );
        assert!(unresolved.blocks_release);
    }

    #[test]
    fn ambiguous_identity_blocks_release() {
        let mut resolved = HashMap::new();
        resolved.insert(
            "7732-18-5".to_string(),
            CasResolution::Ambiguous(vec![
                identity_candidate("7732-18-5", 1, None, None),
                identity_candidate("7732-18-5", 2, None, None),
            ]),
        );
        let result = generate_from_normalized_input(&product(), &resolved, &UnavailableNormalizer);
        assert_eq!(result.release_status, ReleaseStatus::Blocked);
    }

    #[test]
    fn resolved_candidate_with_no_smiles_via_unavailable_normalizer_writes_nothing() {
        let mut resolved = HashMap::new();
        resolved.insert(
            "7732-18-5".to_string(),
            CasResolution::Resolved(identity_candidate("7732-18-5", 962, None, Some("H2O"))),
        );
        let result = generate_from_normalized_input(&product(), &resolved, &UnavailableNormalizer);
        let row = &result
            .sds
            .composition
            .as_ref()
            .unwrap()
            .composition_and_concentration
            .as_ref()
            .unwrap()[0];
        assert!(row.smiles.is_none());
        assert!(result
            .findings
            .iter()
            .any(|f| f.rule == "GEN-STRUCTURE-MISSING"));
    }

    #[test]
    fn no_chematic_result_ever_populates_a_product_level_property() {
        let mut resolved = HashMap::new();
        resolved.insert(
            "7732-18-5".to_string(),
            CasResolution::Resolved(identity_candidate("7732-18-5", 962, Some("O"), Some("H2O"))),
        );
        let result = generate_from_normalized_input(&product(), &resolved, &UnavailableNormalizer);
        assert!(result.sds.physical_chemical_properties.is_none());
        assert!(result.sds.stability_reactivity.is_none());
        assert!(result.sds.hazard_identification.is_none());
    }

    #[test]
    fn official_sds_json_has_no_normalization_report_keys() {
        let mut resolved = HashMap::new();
        resolved.insert(
            "7732-18-5".to_string(),
            CasResolution::Resolved(identity_candidate("7732-18-5", 962, Some("O"), Some("H2O"))),
        );
        let result = generate_from_normalized_input(&product(), &resolved, &UnavailableNormalizer);
        let json = serde_json::to_string(&result.sds).unwrap();
        for leak in [
            "status",
            "issues",
            "screening_alerts",
            "NormalizationStatus",
            "confidence",
        ] {
            assert!(
                !json.contains(leak),
                "official SDS JSON must not contain '{leak}'"
            );
        }
    }

    #[test]
    fn lookup_error_is_nonblocking_and_retains_supplied_data() {
        let mut lookups = HashMap::new();
        lookups.insert("7732-18-5".to_string(), Err("network timeout".to_string()));
        let result = generate_from_detailed_lookups(&product(), &lookups, &UnavailableNormalizer);

        let finding = result
            .findings
            .iter()
            .find(|f| f.rule == "GEN-CAS-LOOKUP-ERROR")
            .expect("expected a GEN-CAS-LOOKUP-ERROR finding");
        assert_eq!(finding.level, "MED");
        assert_ne!(result.release_status, ReleaseStatus::Blocked);

        // Supplied name/CAS/concentration are untouched -- a lookup failure
        // never causes fabricated or discarded composition data.
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
                .substance_identity
                .as_ref()
                .unwrap()
                .ca_sno
                .as_ref()
                .unwrap()
                .full_text
                .as_deref(),
            Some(["7732-18-5".to_string()].as_slice())
        );
    }

    #[test]
    fn lookup_error_is_distinct_from_ambiguous_identity() {
        let mut lookups = HashMap::new();
        lookups.insert("7732-18-5".to_string(), Err("HTTP 503".to_string()));
        let result = generate_from_detailed_lookups(&product(), &lookups, &UnavailableNormalizer);

        assert!(!result
            .findings
            .iter()
            .any(|f| f.rule == "GEN-CAS-AMBIGUOUS"));
        let unresolved = result
            .unresolved
            .iter()
            .find(|f| f.path == path::composition_row(0, path::CAS_NO))
            .unwrap();
        assert_ne!(
            unresolved.reason,
            UnresolvedReason::AmbiguousChemicalIdentity
        );
    }

    #[test]
    fn malformed_cas_lookup_error_does_not_duplicate_format_finding() {
        let mut input = product();
        input.components = vec![exact_component("not-a-cas", "Mystery", 100.0)];
        let mut lookups = HashMap::new();
        lookups.insert("not-a-cas".to_string(), Err("invalid format".to_string()));
        let result = generate_from_detailed_lookups(&input, &lookups, &UnavailableNormalizer);
        assert!(!result
            .findings
            .iter()
            .any(|f| f.rule == "GEN-CAS-LOOKUP-ERROR"));
    }

    #[test]
    fn mixed_lookup_outcomes_are_handled_independently_per_component() {
        let mut input = product();
        input.components = vec![
            exact_component("7732-18-5", "Water", 90.0),
            exact_component("64-17-5", "Ethanol", 10.0),
        ];
        let mut lookups = HashMap::new();
        lookups.insert(
            "7732-18-5".to_string(),
            Ok(CasResolution::Ambiguous(vec![
                identity_candidate("7732-18-5", 1, None, None),
                identity_candidate("7732-18-5", 2, None, None),
            ])),
        );
        lookups.insert("64-17-5".to_string(), Err("connection reset".to_string()));

        let result = generate_from_detailed_lookups(&input, &lookups, &UnavailableNormalizer);
        assert!(result
            .findings
            .iter()
            .any(|f| f.rule == "GEN-CAS-AMBIGUOUS"));
        assert!(result
            .findings
            .iter()
            .any(|f| f.rule == "GEN-CAS-LOOKUP-ERROR"));
        assert_eq!(result.release_status, ReleaseStatus::Blocked); // ambiguity alone blocks
    }

    #[cfg(feature = "chematic-normalization")]
    mod chematic_integration {
        use super::*;
        use crate::normalize::ChematicNormalizer;

        #[test]
        fn matching_formula_populates_smiles_and_keeps_formula() {
            let mut resolved = HashMap::new();
            resolved.insert(
                "7732-18-5".to_string(),
                CasResolution::Resolved(identity_candidate(
                    "7732-18-5",
                    962,
                    Some("O"),
                    Some("H2O"),
                )),
            );
            let result = generate_from_normalized_input(&product(), &resolved, &ChematicNormalizer);
            let row = &result
                .sds
                .composition
                .as_ref()
                .unwrap()
                .composition_and_concentration
                .as_ref()
                .unwrap()[0];
            assert!(row.smiles.is_some());
            assert_eq!(row.molecular_formula.as_deref(), Some("H2O"));
            assert!(!result
                .findings
                .iter()
                .any(|f| f.rule == "GEN-STRUCTURE-FORMULA-MISMATCH"));
        }

        #[test]
        fn formula_mismatch_blocks_release_and_preserves_both_values_in_provenance() {
            let mut resolved = HashMap::new();
            resolved.insert(
                "64-17-5".to_string(),
                CasResolution::Resolved(identity_candidate(
                    "64-17-5",
                    702,
                    Some("CCO"),
                    Some("C6H12O6"),
                )),
            );
            let result =
                generate_from_normalized_input(&product_ethanol(), &resolved, &ChematicNormalizer);

            assert!(result
                .findings
                .iter()
                .any(|f| f.rule == "GEN-STRUCTURE-FORMULA-MISMATCH" && f.level == "HIGH"));
            assert_eq!(result.release_status, ReleaseStatus::Blocked);

            let row = &result
                .sds
                .composition
                .as_ref()
                .unwrap()
                .composition_and_concentration
                .as_ref()
                .unwrap()[0];
            // No conflicting duplicate value exposed in the official field.
            assert!(row.molecular_formula.is_none());

            // Both values retained in the report's provenance.
            let calculated_prov = result
                .provenance
                .iter()
                .find(|p| p.path == path::CANONICAL_SMILES)
                .unwrap();
            assert!(calculated_prov
                .warnings
                .iter()
                .any(|w| w.contains("FormulaMismatch")));
        }

        #[test]
        fn multi_fragment_produces_review_required_not_blocked() {
            let mut resolved = HashMap::new();
            resolved.insert(
                "7647-14-5".to_string(),
                CasResolution::Resolved(identity_candidate(
                    "7647-14-5",
                    5234,
                    Some("[Na+].[Cl-]"),
                    None,
                )),
            );
            let result = generate_from_normalized_input(
                &product_sodium_chloride(),
                &resolved,
                &ChematicNormalizer,
            );
            assert!(result
                .findings
                .iter()
                .any(|f| f.rule == "GEN-STRUCTURE-MULTIFRAGMENT" && f.level == "MED"));
            assert_ne!(result.release_status, ReleaseStatus::Blocked);
            let row = &result
                .sds
                .composition
                .as_ref()
                .unwrap()
                .composition_and_concentration
                .as_ref()
                .unwrap()[0];
            // Structure kept intact -- both fragments present in the written SMILES.
            assert!(row.smiles.as_deref().unwrap().contains('.'));
        }

        #[test]
        fn source_and_canonical_smiles_provenance_never_confirmed() {
            let mut resolved = HashMap::new();
            resolved.insert(
                "7732-18-5".to_string(),
                CasResolution::Resolved(identity_candidate(
                    "7732-18-5",
                    962,
                    Some("O"),
                    Some("H2O"),
                )),
            );
            let result = generate_from_normalized_input(&product(), &resolved, &ChematicNormalizer);
            let source = result
                .provenance
                .iter()
                .find(|p| p.path == path::SOURCE_SMILES)
                .unwrap();
            let canonical = result
                .provenance
                .iter()
                .find(|p| p.path == path::CANONICAL_SMILES)
                .unwrap();
            assert_eq!(source.source_type, EvidenceLevel::ReferenceDatabase);
            assert_eq!(
                canonical.source_type,
                EvidenceLevel::DeterministicCalculation
            );
        }

        #[test]
        fn connectivity_smiles_fallback_reaches_provenance_as_a_warning() {
            let mut candidate = identity_candidate("7732-18-5", 962, None, Some("H2O"));
            candidate.connectivity_smiles = Some("O".into());
            let mut resolved = HashMap::new();
            resolved.insert("7732-18-5".to_string(), CasResolution::Resolved(candidate));
            let result = generate_from_normalized_input(&product(), &resolved, &ChematicNormalizer);
            let source = result
                .provenance
                .iter()
                .find(|p| p.path == path::SOURCE_SMILES)
                .unwrap();
            assert_eq!(
                source.method,
                "PubChem CAS lookup (ConnectivitySMILES fallback)"
            );
            assert!(source
                .warnings
                .iter()
                .any(|w| w.contains("stereochemistry/isotope")));
        }

        #[test]
        fn generation_never_approves_with_normalization() {
            let mut resolved = HashMap::new();
            resolved.insert(
                "7732-18-5".to_string(),
                CasResolution::Resolved(identity_candidate(
                    "7732-18-5",
                    962,
                    Some("O"),
                    Some("H2O"),
                )),
            );
            let result = generate_from_normalized_input(&product(), &resolved, &ChematicNormalizer);
            assert_ne!(result.release_status, ReleaseStatus::Approved);
        }

        #[test]
        fn repeated_generation_with_normalization_is_byte_equivalent() {
            let mut resolved = HashMap::new();
            resolved.insert(
                "7732-18-5".to_string(),
                CasResolution::Resolved(identity_candidate(
                    "7732-18-5",
                    962,
                    Some("O"),
                    Some("H2O"),
                )),
            );
            let a = generate_from_normalized_input(&product(), &resolved, &ChematicNormalizer);
            let b = generate_from_normalized_input(&product(), &resolved, &ChematicNormalizer);
            assert_eq!(
                serde_json::to_string(&a).unwrap(),
                serde_json::to_string(&b).unwrap()
            );
        }

        fn product_ethanol() -> ProductInput {
            let mut p = product();
            p.components[0].cas_number = Some("64-17-5".into());
            p.components[0].name = Some("Ethanol".into());
            p
        }

        fn product_sodium_chloride() -> ProductInput {
            let mut p = product();
            p.components[0].cas_number = Some("7647-14-5".into());
            p.components[0].name = Some("Sodium chloride".into());
            p
        }
    }
}
