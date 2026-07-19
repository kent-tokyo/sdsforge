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
#[serde(deny_unknown_fields)]
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
/// `conditions` deliberately has no `#[serde(default)]`: it must remain a
/// required key so an omitted measurement condition is a parse error, not a
/// silently-accepted empty condition set. (`MeasurementConditions`' own
/// three fields are individually `Option`, so `conditions: {}` is valid —
/// the acceptance policies in `super::resolve` are what reject an
/// insufficiently-specified condition set, not the parser.)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
/// shape as [`MeasuredValueEvidence`], including the required `conditions`
/// key (see that struct's doc comment).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
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
///
/// Container-level `#[serde(default)]`: any subset of the seven fields may
/// be supplied, and the rest default to empty — so
/// `measured_properties: { flash_point: [...] }` is valid without
/// spelling out the other six as `[]`. Combined with `deny_unknown_fields`,
/// a misspelled property name (e.g. `flash_points`) is still a parse
/// error, not a silently-ignored key.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
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

    #[test]
    fn missing_measured_properties_key_becomes_all_empty_collections() {
        let input: MeasuredPropertiesInput = serde_json::from_str("{}").unwrap();
        assert!(input.flash_point.is_empty());
        assert!(input.boiling_point.is_empty());
        assert!(input.vapor_pressure.is_empty());
        assert!(input.explosive_limits.is_empty());
        assert!(input.self_reactivity.is_empty());
        assert!(input.oxidizing_properties.is_empty());
        assert!(input.metal_corrosivity.is_empty());
    }

    #[test]
    fn partial_measured_properties_object_may_contain_only_flash_point() {
        let json = r#"{
            "flash_point": [
                {"value": 12.0, "unit": "degC", "method": null,
                 "conditions": {"temperature_c": 20.0, "pressure_kpa": null, "atmosphere": null},
                 "sample_id": null, "batch_id": "BATCH-001", "evidence_id": "flash-point-report"}
            ]
        }"#;
        let input: MeasuredPropertiesInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.flash_point.len(), 1);
        assert!(input.boiling_point.is_empty());
        assert!(input.vapor_pressure.is_empty());
        assert!(input.explosive_limits.is_empty());
        assert!(input.self_reactivity.is_empty());
        assert!(input.oxidizing_properties.is_empty());
        assert!(input.metal_corrosivity.is_empty());
    }

    #[test]
    fn unknown_measured_property_name_fails() {
        // "flash_points" (extra trailing s) instead of "flash_point".
        let json = r#"{"flash_points": []}"#;
        assert!(serde_json::from_str::<MeasuredPropertiesInput>(json).is_err());
    }

    #[test]
    fn missing_evidence_id_fails() {
        let json = r#"{"level":"product_test_report","reference":"ref",
            "issuer":null,"document_date":null,"applies_to":"finished_product"}"#;
        assert!(serde_json::from_str::<EvidenceSource>(json).is_err());
    }

    #[test]
    fn missing_evidence_reference_fails() {
        let json = r#"{"id":"ev1","level":"product_test_report",
            "issuer":null,"document_date":null,"applies_to":"finished_product"}"#;
        assert!(serde_json::from_str::<EvidenceSource>(json).is_err());
    }

    #[test]
    fn missing_measured_value_evidence_id_fails() {
        let json = r#"{"value":1.0,"unit":"degC","method":null,
            "conditions":{"temperature_c":20.0,"pressure_kpa":null,"atmosphere":null},
            "sample_id":null,"batch_id":null}"#;
        assert!(serde_json::from_str::<MeasuredValueEvidence>(json).is_err());
    }

    #[test]
    fn missing_required_conditions_still_fails() {
        // `conditions` itself is omitted entirely -- not the same as an
        // empty `conditions: {}` object, which would still be valid since
        // every MeasurementConditions field is individually Option.
        let json = r#"{"value":1.0,"unit":"degC","method":null,
            "sample_id":null,"batch_id":null,"evidence_id":"ev1"}"#;
        assert!(serde_json::from_str::<MeasuredValueEvidence>(json).is_err());
    }
}
