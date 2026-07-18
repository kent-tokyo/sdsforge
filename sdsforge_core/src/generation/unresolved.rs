use serde::{Deserialize, Serialize};

use super::provenance::EvidenceLevel;

/// Why a value couldn't be filled in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnresolvedReason {
    MissingInput,
    ProductTestRequired,
    AmbiguousChemicalIdentity,
    ConflictingSources,
    UnsupportedCalculation,
    InsufficientMeasurementConditions,
    MixtureCannotBeDerivedFromComponents,
    RegulatoryJudgementRequired,
    HumanReviewRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SafetyImpact {
    None,
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegulatoryImpact {
    None,
    Low,
    Medium,
    High,
}

/// One piece of evidence a human would need to supply to resolve a field.
/// Kept small — enough to explain what's needed, not a full lab-data model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequiredInput {
    pub name: String,
    pub description: String,
    pub unit: Option<String>,
}

impl RequiredInput {
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        RequiredInput {
            name: name.into(),
            description: description.into(),
            unit: None,
        }
    }

    pub fn with_unit(mut self, unit: impl Into<String>) -> Self {
        self.unit = Some(unit.into());
        self
    }
}

/// A field the generator could not populate, with enough detail for a human
/// (or a downstream system) to know what's missing, why, how bad it is to
/// ship without it, and what evidence would resolve it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnresolvedField {
    pub path: String,
    pub title: String,
    pub reason: UnresolvedReason,
    pub required_inputs: Vec<RequiredInput>,
    pub acceptable_evidence: Vec<EvidenceLevel>,
    pub safety_impact: SafetyImpact,
    pub regulatory_impact: RegulatoryImpact,
    pub recommended_action: String,
    pub blocks_release: bool,
}

/// Placeholder for a deliberate "this field does not apply to this product"
/// determination. Not constructed anywhere in this commit's generation
/// logic — see `docs/sdsforge-architecture.md`'s generation-architecture
/// section: when applicability genuinely can't be determined (e.g. this
/// commit's `ProductInput` carries no physical-state field), the correct
/// reason is [`UnresolvedReason::HumanReviewRequired`], not a guessed
/// `NotApplicable`. Defined now so [`super::FieldStatus`] compiles and so a
/// future commit that *can* determine applicability has a type to use.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotApplicableReason {
    pub explanation: String,
}

/// How a field's value was determined. `Calculated`/`Estimated`/`Literature`
/// must never auto-promote to `Confirmed` — promotion requires new
/// evidence, not a higher-confidence label on the same evidence.
///
/// Not currently stored on [`super::GenerationResult`] (which populates
/// `SdsRoot` fields directly, per commit #9's design) — defined per the
/// architecture spec's type system and used internally by evidence-summary
/// computation (`super::result::evidence_level_bucket`) to categorize each
/// [`super::provenance::FieldProvenance`] record.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldStatus<T> {
    Confirmed(T),
    Supplied(T),
    Literature(T),
    Calculated(T),
    Estimated(T),
    Unresolved(UnresolvedField),
    NotApplicable(NotApplicableReason),
}

/// What evidence is even eligible for a given field, and whether missing it
/// blocks release. Data, not scattered conditionals — see
/// `PRODUCT_LEVEL_POLICIES` below.
#[derive(Debug, Clone)]
pub struct FieldPolicy {
    pub path: &'static str,
    pub allowed_evidence: &'static [EvidenceLevel],
    pub product_test_required: bool,
    pub calculation_allowed: bool,
    pub estimation_allowed: bool,
    pub blocks_release_if_missing: bool,
}

/// Molecular formula as returned by [`crate::enrichment::lookup_cas`]: an
/// identity candidate, reference-database evidence, no product test
/// required. Included for completeness of the policy table — commit #9
/// already populates this field when the resolver supplies it; this policy
/// documents the evidence rule that governs it.
pub const MOLECULAR_FORMULA_POLICY: FieldPolicy = FieldPolicy {
    path: "Composition.CompositionAndConcentration[].MolecularFormula",
    allowed_evidence: &[
        EvidenceLevel::ReferenceDatabase,
        EvidenceLevel::DeterministicCalculation,
    ],
    product_test_required: false,
    calculation_allowed: true,
    estimation_allowed: false,
    blocks_release_if_missing: false,
};

/// Product-level physical/hazard properties that Commit #9's Section 1/3
/// draft never populates — CAS identity and composition alone are not
/// sufficient evidence for any of these (see
/// `docs/sdsforge-architecture.md`'s "Properties that require product-level
/// evidence" table). Every one of these becomes an [`UnresolvedField`] on
/// every generation result, since `ProductInput` (commit #8) supplies none
/// of the underlying test data.
pub const PRODUCT_LEVEL_POLICIES: &[FieldPolicy] = &[
    FieldPolicy {
        path: "PhysicalChemicalProperties.FlashPoint",
        allowed_evidence: &[
            EvidenceLevel::ProductTestReport,
            EvidenceLevel::EquivalentBatchTestReport,
            EvidenceLevel::SupplierSpecification,
        ],
        product_test_required: true,
        calculation_allowed: false,
        estimation_allowed: false,
        blocks_release_if_missing: false, // see build_product_level_unresolved: applicability is undetermined, not confirmed-blocking
    },
    FieldPolicy {
        path: "PhysicalChemicalProperties.InitialBoilingPointAndBoilingRange",
        allowed_evidence: &[
            EvidenceLevel::ProductTestReport,
            EvidenceLevel::EquivalentBatchTestReport,
        ],
        product_test_required: true,
        calculation_allowed: false,
        estimation_allowed: false,
        blocks_release_if_missing: false,
    },
    FieldPolicy {
        path: "PhysicalChemicalProperties.VapourPressure",
        allowed_evidence: &[
            EvidenceLevel::ProductTestReport,
            EvidenceLevel::EquivalentBatchTestReport,
        ],
        product_test_required: true,
        calculation_allowed: false,
        estimation_allowed: false,
        blocks_release_if_missing: false,
    },
    FieldPolicy {
        path: "PhysicalChemicalProperties.ExplosiveLimits",
        allowed_evidence: &[
            EvidenceLevel::ProductTestReport,
            EvidenceLevel::EquivalentBatchTestReport,
        ],
        product_test_required: true,
        calculation_allowed: false,
        estimation_allowed: false,
        blocks_release_if_missing: false,
    },
    FieldPolicy {
        path: "StabilityReactivity.SelfReactivity",
        allowed_evidence: &[
            EvidenceLevel::ProductTestReport,
            EvidenceLevel::EquivalentBatchTestReport,
        ],
        product_test_required: true,
        calculation_allowed: false,
        estimation_allowed: false,
        blocks_release_if_missing: false,
    },
    FieldPolicy {
        path: "PhysicalChemicalProperties.OxidizingProperties",
        allowed_evidence: &[
            EvidenceLevel::ProductTestReport,
            EvidenceLevel::EquivalentBatchTestReport,
        ],
        product_test_required: true,
        calculation_allowed: false,
        estimation_allowed: false,
        blocks_release_if_missing: false,
    },
    FieldPolicy {
        path: "HazardIdentification.Classification.PhysicochemicalEffect.CorrosiveToMetals",
        allowed_evidence: &[
            EvidenceLevel::ProductTestReport,
            EvidenceLevel::EquivalentBatchTestReport,
        ],
        product_test_required: true,
        calculation_allowed: false,
        estimation_allowed: false,
        blocks_release_if_missing: false,
    },
];

/// Builds the always-present product-level [`UnresolvedField`]s.
///
/// Every one uses [`UnresolvedReason::HumanReviewRequired`], not
/// `ProductTestRequired` — `ProductInput` carries no physical-state field,
/// so the generator cannot determine whether e.g. flash point is even
/// applicable to this product before a human looks at it. Pretending
/// otherwise (guessing "liquid, therefore blocking" or "solid, therefore
/// not applicable") would be exactly the kind of applicability-guessing
/// this feature avoids; a human must resolve applicability first, and
/// `required_inputs` names the test evidence needed once they do.
pub fn build_product_level_unresolved() -> Vec<UnresolvedField> {
    PRODUCT_LEVEL_POLICIES
        .iter()
        .map(|policy| {
            let (title, required_inputs) = product_level_detail(policy.path);
            UnresolvedField {
                path: policy.path.to_string(),
                title,
                reason: UnresolvedReason::HumanReviewRequired,
                required_inputs,
                acceptable_evidence: policy.allowed_evidence.to_vec(),
                safety_impact: SafetyImpact::Medium,
                regulatory_impact: RegulatoryImpact::Medium,
                recommended_action:
                    "A human must first determine whether this property applies to \
                     the product (ProductInput carries no physical-state/use information), then \
                     supply product or equivalent-batch test evidence if it does."
                        .to_string(),
                blocks_release: policy.blocks_release_if_missing,
            }
        })
        .collect()
}

pub(super) fn product_level_detail(path: &str) -> (String, Vec<RequiredInput>) {
    match path {
        "PhysicalChemicalProperties.FlashPoint" => (
            "Flash point".into(),
            vec![
                RequiredInput::new("value", "Measured flash point").with_unit("°C"),
                RequiredInput::new("method", "Open-cup or closed-cup test method"),
                RequiredInput::new("sample_or_batch_id", "Sample or batch identity tested"),
            ],
        ),
        "PhysicalChemicalProperties.InitialBoilingPointAndBoilingRange" => (
            "Initial boiling point / boiling range".into(),
            vec![
                RequiredInput::new("value", "Boiling point or range").with_unit("°C"),
                RequiredInput::new("pressure", "Pressure at which measured").with_unit("kPa"),
                RequiredInput::new("method", "Test method"),
                RequiredInput::new(
                    "decomposes_before_boiling",
                    "Whether the substance decomposes before boiling",
                ),
            ],
        ),
        "PhysicalChemicalProperties.VapourPressure" => (
            "Vapour pressure".into(),
            vec![
                RequiredInput::new("value", "Vapour pressure").with_unit("kPa"),
                RequiredInput::new(
                    "measurement_temperature",
                    "Temperature at which vapour pressure was measured — a vapour pressure value \
                     without its measurement temperature is not usable",
                )
                .with_unit("°C"),
                RequiredInput::new("basis", "Whether the value is measured or calculated"),
            ],
        ),
        "PhysicalChemicalProperties.ExplosiveLimits" => (
            "Explosive (flammability) limits".into(),
            vec![
                RequiredInput::new("lower_limit", "Lower explosive limit").with_unit("vol %"),
                RequiredInput::new("upper_limit", "Upper explosive limit").with_unit("vol %"),
                RequiredInput::new("atmosphere", "Test atmosphere / O2 percentage"),
                RequiredInput::new("temperature", "Test temperature").with_unit("°C"),
                RequiredInput::new("method", "Test method"),
            ],
        ),
        "StabilityReactivity.SelfReactivity" => (
            "Self-reactivity".into(),
            vec![RequiredInput::new(
                "test_result",
                "UN test series A-H result, or self-accelerating decomposition temperature (SADT) — \
                 DSC screening alone is not sufficient",
            )],
        ),
        "PhysicalChemicalProperties.OxidizingProperties" => (
            "Oxidizing properties".into(),
            vec![RequiredInput::new(
                "test_result",
                "Physical-state-appropriate UN test result (O.1/O.2/O.3) — structure alone does not \
                 resolve this field",
            )],
        ),
        "HazardIdentification.Classification.PhysicochemicalEffect.CorrosiveToMetals" => (
            "Corrosive to metals".into(),
            vec![RequiredInput::new(
                "corrosion_rate_test",
                "Steel/aluminium corrosion-rate test at a specified temperature — pH alone is \
                 insufficient",
            )
            .with_unit("mm/year")],
        ),
        _ => (path.to_string(), Vec::new()),
    }
}

/// Builds an [`UnresolvedField`] for a component whose CAS enrichment
/// lookup did not resolve (mirrors commit #9's `GEN-CAS-ENRICHMENT-MISSING`
/// finding, which is retained alongside this — a `Finding` is a severity-
/// bearing diagnostic event, this is a field-level statement of what's
/// missing and how to fix it; they serve different purposes and neither
/// replaces the other).
///
/// Always `UnresolvedReason::MissingInput`, never a more specific reason
/// like `AmbiguousChemicalIdentity` — the pure mapping function this feeds
/// from (`draft_sections_from_resolved_input`) only sees a `HashMap<String,
/// CasInfo>` of *successful* lookups (commit #9's frozen signature), so it
/// cannot distinguish "not found in PubChem" from "network/parse error" for
/// a missing entry. This is a known, deliberate limitation — recorded here
/// rather than redesigning `enrichment::lookup_cas`'s error type in this
/// commit.
///
/// `blocks_release` is always `false`: the supplied name/CAS/concentration
/// remain in the draft regardless (commit #9's guarantee), so the
/// composition row is usable even without enrichment. Nothing in
/// `ProductInput` gives this function a way to detect "the missing
/// identity makes the composition materially unsafe or unusable" — that
/// judgement call is left to human review, not guessed here.
pub fn build_lookup_failure_unresolved(
    component_index: usize,
    cas: &str,
    name: Option<&str>,
) -> UnresolvedField {
    let label = name
        .map(|n| format!("'{n}' (CAS {cas})"))
        .unwrap_or_else(|| format!("CAS {cas}"));
    UnresolvedField {
        path: super::provenance::path::composition_row(
            component_index,
            super::provenance::path::CAS_NO,
        ),
        title: format!("Chemical identity for component {label}"),
        reason: UnresolvedReason::MissingInput,
        required_inputs: vec![RequiredInput::new(
            "authoritative_identity_source",
            "An authoritative source confirming this CAS number's identity — automatic lookup did \
             not return a match. This may mean the record wasn't found, or the lookup request \
             itself failed; the two are not currently distinguished.",
        )],
        acceptable_evidence: vec![
            EvidenceLevel::SupplierSpecification,
            EvidenceLevel::SupplierSds,
            EvidenceLevel::RegulatoryDatabase,
            EvidenceLevel::ReferenceDatabase,
        ],
        safety_impact: SafetyImpact::Low,
        regulatory_impact: RegulatoryImpact::Low,
        recommended_action: "Verify the chemical identity and provide an authoritative source \
            (e.g. supplier SDS, regulatory database entry)."
            .into(),
        blocks_release: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn product_level_unresolved_covers_all_seven_properties() {
        let unresolved = build_product_level_unresolved();
        assert_eq!(unresolved.len(), 7);
        assert!(unresolved
            .iter()
            .all(|f| f.reason == UnresolvedReason::HumanReviewRequired));
        assert!(unresolved.iter().all(|f| !f.blocks_release));
    }

    #[test]
    fn flash_point_required_inputs_are_specific() {
        let unresolved = build_product_level_unresolved();
        let flash_point = unresolved
            .iter()
            .find(|f| f.path == "PhysicalChemicalProperties.FlashPoint")
            .unwrap();
        assert!(flash_point
            .required_inputs
            .iter()
            .any(|i| i.name == "method"));
    }

    #[test]
    fn vapour_pressure_requires_measurement_temperature() {
        let unresolved = build_product_level_unresolved();
        let vp = unresolved
            .iter()
            .find(|f| f.path == "PhysicalChemicalProperties.VapourPressure")
            .unwrap();
        assert!(vp
            .required_inputs
            .iter()
            .any(|i| i.name == "measurement_temperature"));
    }

    #[test]
    fn metal_corrosivity_states_ph_alone_is_insufficient() {
        let unresolved = build_product_level_unresolved();
        let corrosivity = unresolved
            .iter()
            .find(|f| {
                f.path
                    == "HazardIdentification.Classification.PhysicochemicalEffect.CorrosiveToMetals"
            })
            .unwrap();
        assert!(corrosivity.required_inputs[0]
            .description
            .contains("pH alone"));
    }

    #[test]
    fn no_component_averaging_language_anywhere() {
        // Structural check: nothing in this module computes an average —
        // grep-style assertion that the self-reactivity/oxidizing/flash
        // point policies require a direct test result, not a formula.
        for policy in PRODUCT_LEVEL_POLICIES {
            assert!(!policy.calculation_allowed || policy.path.contains("MolecularFormula"));
        }
    }

    #[test]
    fn lookup_failure_unresolved_never_blocks_release() {
        let field = build_lookup_failure_unresolved(0, "7732-18-5", Some("Water"));
        assert!(!field.blocks_release);
        assert_eq!(field.reason, UnresolvedReason::MissingInput);
    }

    #[test]
    fn unresolved_reason_serializes_snake_case() {
        let json = serde_json::to_string(&UnresolvedReason::ProductTestRequired).unwrap();
        assert_eq!(json, "\"product_test_required\"");
    }
}
