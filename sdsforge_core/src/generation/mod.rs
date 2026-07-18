//! Formulation input domain model and draft generation for the `generate`
//! feature.
//!
//! Roadmap (see `docs/sdsforge-architecture.md`'s "Generation architecture"
//! section):
//! - #8 `input`/`validate` — `ProductInput` and friends, deterministic
//!   offline validation.
//! - #9 `draft` — maps a validated `ProductInput` to a partial `SdsRoot`
//!   (Section 1/3 only).
//! - #10 `provenance`/`unresolved`/`result` — wraps #9's draft in a
//!   [`GenerationResult`] that explains what's populated, where it came
//!   from, what's still missing, and whether the draft is releasable.
//! - #11+ chematic integration, CLI, GUI — not yet implemented.

mod draft;
mod evidence;
mod input;
mod provenance;
mod resolve;
mod result;
mod unresolved;
mod validate;

pub use draft::{draft_sections_from_resolved_input, generate_section_1_and_3, SectionDraftResult};
pub use evidence::{
    EvidenceApplicability, EvidenceSource, ExplosiveLimitsEvidence, MeasuredPropertiesInput,
    MeasuredValueEvidence, TestResultEvidence,
};
pub use input::{ComponentInput, ConcentrationRange, ProductInput, SupplierInput};
pub use provenance::{
    path as field_path, ConfidenceLevel, EvidenceLevel, FieldProvenance, MeasurementConditions,
};
pub use result::{
    compute_evidence_summary, compute_release_status, evaluate_release_gate,
    generate_from_normalized_input, generate_from_resolved_input, generate_with_enrichment,
    EvidenceSummary, GenerationResult, ReleaseGateResult, ReleaseStatus,
};
pub use unresolved::{
    build_product_level_unresolved, FieldPolicy, FieldStatus, NotApplicableReason,
    RegulatoryImpact, RequiredInput, SafetyImpact, UnresolvedField, UnresolvedReason,
    MOLECULAR_FORMULA_POLICY, PRODUCT_LEVEL_POLICIES,
};
pub use validate::validate_product_input;
