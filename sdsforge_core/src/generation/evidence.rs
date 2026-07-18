use serde::{Deserialize, Serialize};

use super::provenance::{EvidenceLevel, MeasurementConditions};

/// What the evidence actually was measured on — a mixture property can
/// legitimately be confirmed by finished-product or same/equivalent-batch
/// evidence, but never by component-level or reference-substance data (see
/// `super::result`'s evidence-resolution logic).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceApplicability {
    FinishedProduct,
    SameBatch,
    EquivalentBatch,
    Component,
    ReferenceSubstance,
    Unknown,
}

/// A reference to a piece of supporting evidence — a test report, a
/// supplier specification, a database entry. Stores identifying metadata
/// only (a reference string, issuer, date); never the document itself or
/// arbitrary binary data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceSource {
    /// Referenced by `evidence_id` on the measured-value types below.
    pub id: String,
    pub level: EvidenceLevel,
    pub reference: String,
    pub issuer: Option<String>,
    pub document_date: Option<String>,
    pub applies_to: EvidenceApplicability,
}

/// A measured value for one of the four properties whose MHLW schema field
/// has a real numeric+unit shape (flash point, boiling point, vapor
/// pressure). `evidence_id` must match an [`EvidenceSource::id`] in
/// [`super::ProductInput::evidence`] — a value with no resolvable evidence
/// reference can never become `Confirmed`, only `Supplied` at best (see
/// resolution logic in `super::result`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeasuredValueEvidence {
    pub value: f64,
    pub unit: String,
    pub method: Option<String>,
    pub conditions: MeasurementConditions,
    pub sample_id: Option<String>,
    pub batch_id: Option<String>,
    pub evidence_id: String,
}

/// Explosive (flammability) limits need two bounds; otherwise the same
/// shape as [`MeasuredValueEvidence`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplosiveLimitsEvidence {
    pub lower: Option<f64>,
    pub upper: Option<f64>,
    pub unit: String,
    pub method: Option<String>,
    pub conditions: MeasurementConditions,
    pub sample_id: Option<String>,
    pub batch_id: Option<String>,
    pub evidence_id: String,
}

/// For the three properties whose MHLW schema field is free text with no
/// structured numeric value at all (self-reactivity, oxidizing properties,
/// metal corrosivity) — `result` describes the test outcome directly
/// rather than a value+unit pair, since that's the actual shape of the
/// target field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResultEvidence {
    pub result: String,
    pub method: Option<String>,
    pub sample_id: Option<String>,
    pub batch_id: Option<String>,
    pub evidence_id: String,
}

/// All measured-property evidence a caller supplies for the seven
/// properties [`super::unresolved::PRODUCT_LEVEL_POLICIES`] covers. Each
/// field is a `Vec`, not an `Option` — zero entries means nothing was
/// supplied (stays `Unresolved`/`HumanReviewRequired`, unchanged from
/// commit #10); more than one entry makes disagreement between reports
/// representable, which a single `Option` slot could never express.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MeasuredPropertiesInput {
    pub flash_point: Vec<MeasuredValueEvidence>,
    pub boiling_point: Vec<MeasuredValueEvidence>,
    pub vapor_pressure: Vec<MeasuredValueEvidence>,
    pub explosive_limits: Vec<ExplosiveLimitsEvidence>,
    pub self_reactivity: Vec<TestResultEvidence>,
    pub oxidizing_properties: Vec<TestResultEvidence>,
    pub metal_corrosivity: Vec<TestResultEvidence>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measured_properties_input_defaults_to_all_empty() {
        let input = MeasuredPropertiesInput::default();
        assert!(input.flash_point.is_empty());
        assert!(input.boiling_point.is_empty());
        assert!(input.vapor_pressure.is_empty());
        assert!(input.explosive_limits.is_empty());
        assert!(input.self_reactivity.is_empty());
        assert!(input.oxidizing_properties.is_empty());
        assert!(input.metal_corrosivity.is_empty());
    }

    #[test]
    fn evidence_applicability_serializes_snake_case() {
        let json = serde_json::to_string(&EvidenceApplicability::EquivalentBatch).unwrap();
        assert_eq!(json, "\"equivalent_batch\"");
    }
}
