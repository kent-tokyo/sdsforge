//! Evidence-eligibility resolution for the seven safety-sensitive
//! properties Section 1/3 generation (commit #9) can never derive on its
//! own. A property moves from `Unresolved` to written-into-`SdsRoot` only
//! when its supplied evidence fully satisfies the property's
//! [`FieldPolicy`] — never from a bare number, never from component-level
//! data, never by silently picking one of several disagreeing reports.
//! "No partial credit": every rejection path below returns an
//! `UnresolvedField`, just with a progressively more specific reason than
//! commit #10 could give; nothing is written to `SdsRoot` unless resolution
//! fully succeeds.

use crate::schema::{
    HazardIdentificationClassificationPhysicochemicalEffect, NumericRangeWithUnitAndQualifier,
    NumericRangeWithUnitAndQualifierExactValue, PhysicalChemicalPropertiesBoilingPointRelated,
    PhysicalChemicalPropertiesBoilingPointRelatedCondition,
    PhysicalChemicalPropertiesExplosionLimit, PhysicalChemicalPropertiesFlashPoint,
    PhysicalChemicalPropertiesVapourPressure, PhysicalChemicalPropertiesVapourPressureCondition,
    SdsRoot, StabilityReactivityHazardousReactions,
};

use super::evidence::{
    EvidenceApplicability, EvidenceSource, ExplosiveLimitsEvidence, MeasuredValueEvidence,
    TestResultEvidence,
};
use super::input::ProductInput;
use super::provenance::{FieldProvenance, MeasurementConditions};
use super::unresolved::{
    product_level_detail, FieldPolicy, RegulatoryImpact, SafetyImpact, UnresolvedField,
    UnresolvedReason, PRODUCT_LEVEL_POLICIES,
};

const FLASH_POINT_PATH: &str = "PhysicalChemicalProperties.FlashPoint";
const BOILING_POINT_PATH: &str = "PhysicalChemicalProperties.InitialBoilingPointAndBoilingRange";
const VAPOR_PRESSURE_PATH: &str = "PhysicalChemicalProperties.VapourPressure";
const EXPLOSIVE_LIMITS_PATH: &str = "PhysicalChemicalProperties.ExplosiveLimits";
const SELF_REACTIVITY_PATH: &str = "StabilityReactivity.SelfReactivity";
const OXIDIZING_PROPERTIES_PATH: &str = "PhysicalChemicalProperties.OxidizingProperties";
const METAL_CORROSIVITY_PATH: &str =
    "HazardIdentification.Classification.PhysicochemicalEffect.CorrosiveToMetals";

fn policy(path: &str) -> &'static FieldPolicy {
    PRODUCT_LEVEL_POLICIES
        .iter()
        .find(|p| p.path == path)
        .unwrap_or_else(|| panic!("no FieldPolicy registered for {path}"))
}

/// Resolves all seven properties against `input.measured_properties` +
/// `input.evidence`, writing confirmed values into `sds` and returning the
/// rest as [`UnresolvedField`]s plus [`FieldProvenance`] for whatever did
/// resolve. Internal to `generation` — called from `result::generate_from_resolved_input`,
/// not part of the module's public API (it mutates an already-partially-built
/// `SdsRoot` in place, an awkward shape for a public entry point).
pub(super) fn resolve_measured_properties(
    input: &ProductInput,
    sds: &mut SdsRoot,
) -> (Vec<UnresolvedField>, Vec<FieldProvenance>) {
    let mut unresolved = Vec::new();
    let mut provenance = Vec::new();
    let mp = &input.measured_properties;
    let evidence = &input.evidence;

    match resolve_measured_value(&mp.flash_point, evidence, policy(FLASH_POINT_PATH), false) {
        Ok((mv, source)) => {
            apply_flash_point(sds, mv);
            provenance.push(FieldProvenance::from_measured_evidence(
                FLASH_POINT_PATH,
                source,
                mv.method.as_deref(),
                mv.sample_id.as_deref(),
                mv.batch_id.as_deref(),
                &mv.conditions,
            ));
        }
        Err(reason) => unresolved.push(unresolved_for(policy(FLASH_POINT_PATH), reason)),
    }

    match resolve_measured_value(
        &mp.boiling_point,
        evidence,
        policy(BOILING_POINT_PATH),
        false,
    ) {
        Ok((mv, source)) => {
            apply_boiling_point(sds, mv);
            provenance.push(FieldProvenance::from_measured_evidence(
                BOILING_POINT_PATH,
                source,
                mv.method.as_deref(),
                mv.sample_id.as_deref(),
                mv.batch_id.as_deref(),
                &mv.conditions,
            ));
        }
        Err(reason) => unresolved.push(unresolved_for(policy(BOILING_POINT_PATH), reason)),
    }

    // Vapor pressure is the one property with an explicit, schema-structured
    // completeness requirement: a pressure value without its measurement
    // temperature is not usable, even if the evidence is otherwise eligible.
    match resolve_measured_value(
        &mp.vapor_pressure,
        evidence,
        policy(VAPOR_PRESSURE_PATH),
        true,
    ) {
        Ok((mv, source)) => {
            apply_vapor_pressure(sds, mv);
            provenance.push(FieldProvenance::from_measured_evidence(
                VAPOR_PRESSURE_PATH,
                source,
                mv.method.as_deref(),
                mv.sample_id.as_deref(),
                mv.batch_id.as_deref(),
                &mv.conditions,
            ));
        }
        Err(reason) => unresolved.push(unresolved_for(policy(VAPOR_PRESSURE_PATH), reason)),
    }

    match resolve_explosive_limits(
        &mp.explosive_limits,
        evidence,
        policy(EXPLOSIVE_LIMITS_PATH),
    ) {
        Ok((ev, source)) => {
            apply_explosive_limits(sds, ev);
            provenance.push(FieldProvenance::from_measured_evidence(
                EXPLOSIVE_LIMITS_PATH,
                source,
                ev.method.as_deref(),
                ev.sample_id.as_deref(),
                ev.batch_id.as_deref(),
                &ev.conditions,
            ));
        }
        Err(reason) => unresolved.push(unresolved_for(policy(EXPLOSIVE_LIMITS_PATH), reason)),
    }

    match resolve_test_result(&mp.self_reactivity, evidence, policy(SELF_REACTIVITY_PATH)) {
        Ok((tr, source)) => {
            apply_self_reactivity(sds, tr);
            provenance.push(FieldProvenance::from_test_result_evidence(
                SELF_REACTIVITY_PATH,
                source,
                tr,
            ));
        }
        Err(reason) => unresolved.push(unresolved_for(policy(SELF_REACTIVITY_PATH), reason)),
    }

    match resolve_test_result(
        &mp.oxidizing_properties,
        evidence,
        policy(OXIDIZING_PROPERTIES_PATH),
    ) {
        Ok((tr, source)) => {
            apply_oxidizing_properties(sds, tr);
            provenance.push(FieldProvenance::from_test_result_evidence(
                OXIDIZING_PROPERTIES_PATH,
                source,
                tr,
            ));
        }
        Err(reason) => unresolved.push(unresolved_for(policy(OXIDIZING_PROPERTIES_PATH), reason)),
    }

    match resolve_test_result(
        &mp.metal_corrosivity,
        evidence,
        policy(METAL_CORROSIVITY_PATH),
    ) {
        Ok((tr, source)) => {
            apply_metal_corrosivity(sds, tr);
            provenance.push(FieldProvenance::from_test_result_evidence(
                METAL_CORROSIVITY_PATH,
                source,
                tr,
            ));
        }
        Err(reason) => unresolved.push(unresolved_for(policy(METAL_CORROSIVITY_PATH), reason)),
    }

    (unresolved, provenance)
}

/// Checks whether `evidence_id` resolves to an [`EvidenceSource`] eligible
/// under `policy` — steps 3-5 of the resolution order (evidence exists,
/// isn't component/reference-substance-level, and its [`EvidenceLevel`] is
/// on the policy's allow-list). Shared by all three value shapes below.
fn eligible_source<'a>(
    evidence_id: &str,
    evidence: &'a [EvidenceSource],
    policy: &FieldPolicy,
) -> Result<&'a EvidenceSource, UnresolvedReason> {
    let Some(source) = evidence.iter().find(|e| e.id == evidence_id) else {
        return Err(UnresolvedReason::MissingInput);
    };
    match source.applies_to {
        // The literal case the brief asked for: component-level evidence
        // must never confirm a mixture property.
        EvidenceApplicability::Component => {
            return Err(UnresolvedReason::MixtureCannotBeDerivedFromComponents)
        }
        EvidenceApplicability::ReferenceSubstance | EvidenceApplicability::Unknown => {
            return Err(UnresolvedReason::ProductTestRequired)
        }
        EvidenceApplicability::FinishedProduct
        | EvidenceApplicability::SameBatch
        | EvidenceApplicability::EquivalentBatch => {}
    }
    if !policy.allowed_evidence.contains(&source.level) {
        return Err(UnresolvedReason::ProductTestRequired);
    }
    Ok(source)
}

fn resolve_measured_value<'a>(
    entries: &'a [MeasuredValueEvidence],
    evidence: &'a [EvidenceSource],
    policy: &FieldPolicy,
    requires_temperature: bool,
) -> Result<(&'a MeasuredValueEvidence, &'a EvidenceSource), UnresolvedReason> {
    if entries.is_empty() {
        return Err(UnresolvedReason::HumanReviewRequired);
    }
    let mut eligible = Vec::new();
    let mut first_err = None;
    for e in entries {
        match eligible_source(&e.evidence_id, evidence, policy) {
            Ok(source) => eligible.push((e, source)),
            Err(reason) => {
                first_err.get_or_insert(reason);
            }
        };
    }
    if eligible.is_empty() {
        return Err(first_err.unwrap_or(UnresolvedReason::MissingInput));
    }
    if eligible.len() > 1 && eligible.iter().any(|(e, _)| e.value != eligible[0].0.value) {
        return Err(UnresolvedReason::ConflictingSources);
    }
    let (chosen, source) = eligible[0];
    // A measured value with no documented test method isn't confirmable —
    // e.g. a flash point number is materially different depending on
    // open-cup vs. closed-cup method, so "some number, no method" is
    // exactly the kind of "looks complete but is wrong" output this
    // feature avoids.
    if chosen.method.is_none() {
        return Err(UnresolvedReason::InsufficientMeasurementConditions);
    }
    if requires_temperature && chosen.conditions.temperature_c.is_none() {
        return Err(UnresolvedReason::InsufficientMeasurementConditions);
    }
    Ok((chosen, source))
}

fn resolve_explosive_limits<'a>(
    entries: &'a [ExplosiveLimitsEvidence],
    evidence: &'a [EvidenceSource],
    policy: &FieldPolicy,
) -> Result<(&'a ExplosiveLimitsEvidence, &'a EvidenceSource), UnresolvedReason> {
    if entries.is_empty() {
        return Err(UnresolvedReason::HumanReviewRequired);
    }
    let mut eligible = Vec::new();
    let mut first_err = None;
    for e in entries {
        match eligible_source(&e.evidence_id, evidence, policy) {
            Ok(source) => eligible.push((e, source)),
            Err(reason) => {
                first_err.get_or_insert(reason);
            }
        };
    }
    if eligible.is_empty() {
        return Err(first_err.unwrap_or(UnresolvedReason::MissingInput));
    }
    if eligible.len() > 1
        && eligible
            .iter()
            .any(|(e, _)| e.lower != eligible[0].0.lower || e.upper != eligible[0].0.upper)
    {
        return Err(UnresolvedReason::ConflictingSources);
    }
    if eligible[0].0.method.is_none() {
        return Err(UnresolvedReason::InsufficientMeasurementConditions);
    }
    Ok(eligible[0])
}

fn resolve_test_result<'a>(
    entries: &'a [TestResultEvidence],
    evidence: &'a [EvidenceSource],
    policy: &FieldPolicy,
) -> Result<(&'a TestResultEvidence, &'a EvidenceSource), UnresolvedReason> {
    if entries.is_empty() {
        return Err(UnresolvedReason::HumanReviewRequired);
    }
    let mut eligible = Vec::new();
    let mut first_err = None;
    for e in entries {
        match eligible_source(&e.evidence_id, evidence, policy) {
            Ok(source) => eligible.push((e, source)),
            Err(reason) => {
                first_err.get_or_insert(reason);
            }
        };
    }
    if eligible.is_empty() {
        return Err(first_err.unwrap_or(UnresolvedReason::MissingInput));
    }
    if eligible.len() > 1
        && eligible
            .iter()
            .any(|(e, _)| e.result != eligible[0].0.result)
    {
        return Err(UnresolvedReason::ConflictingSources);
    }
    if eligible[0].0.method.is_none() {
        return Err(UnresolvedReason::InsufficientMeasurementConditions);
    }
    Ok(eligible[0])
}

fn unresolved_for(policy: &FieldPolicy, reason: UnresolvedReason) -> UnresolvedField {
    let (title, required_inputs) = product_level_detail(policy.path);
    let recommended_action = match reason {
        UnresolvedReason::HumanReviewRequired => {
            "A human must first determine whether this property applies to the product \
             (ProductInput carries no physical-state/use information), then supply product or \
             equivalent-batch test evidence if it does."
                .to_string()
        }
        UnresolvedReason::MissingInput => {
            "A value was supplied but its evidence_id does not match any EvidenceSource in \
             ProductInput.evidence — provide a resolvable evidence reference."
                .to_string()
        }
        UnresolvedReason::ProductTestRequired => {
            "Supplied evidence is not eligible for this field (wrong evidence level or \
             applicability for this property) — provide product or equivalent-batch test evidence."
                .to_string()
        }
        UnresolvedReason::MixtureCannotBeDerivedFromComponents => {
            "Supplied evidence applies to a single component, not the finished product — a \
             mixture property cannot be confirmed from component-level data alone."
                .to_string()
        }
        UnresolvedReason::InsufficientMeasurementConditions => {
            "Supplied evidence is missing a required measurement condition (e.g. temperature) — \
             resupply with complete conditions."
                .to_string()
        }
        UnresolvedReason::ConflictingSources => {
            "Multiple eligible evidence sources disagree on this value — resolve the discrepancy \
             before it can be confirmed."
                .to_string()
        }
        _ => "Human review required.".to_string(),
    };
    UnresolvedField {
        path: policy.path.to_string(),
        title,
        reason,
        required_inputs,
        acceptable_evidence: policy.allowed_evidence.to_vec(),
        safety_impact: SafetyImpact::Medium,
        regulatory_impact: RegulatoryImpact::Medium,
        recommended_action,
        // ConflictingSources always blocks release, regardless of this
        // property's base policy — shipping a draft that silently picked
        // one of several disagreeing reports would be exactly the kind of
        // "looks complete but is wrong" output this feature exists to
        // prevent.
        blocks_release: matches!(reason, UnresolvedReason::ConflictingSources)
            || policy.blocks_release_if_missing,
    }
}

fn numeric_exact(value: f64, unit: &str) -> NumericRangeWithUnitAndQualifier {
    NumericRangeWithUnitAndQualifier {
        exact_value: Some(NumericRangeWithUnitAndQualifierExactValue {
            value_symbol: None,
            value: Some(value),
        }),
        upper_value: None,
        lower_value: None,
        unit: Some(unit.to_string()),
        additional_info: None,
    }
}

/// Folds temperature/pressure/atmosphere into a free-text description, for
/// the schema fields (flash point, explosive limits) that have no
/// structured condition fields of their own.
fn format_conditions(c: &MeasurementConditions) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(t) = c.temperature_c {
        parts.push(format!("{t}°C"));
    }
    if let Some(p) = c.pressure_kpa {
        parts.push(format!("{p} kPa"));
    }
    if let Some(a) = &c.atmosphere {
        parts.push(a.clone());
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(", "))
    }
}

fn describe_test_result(tr: &TestResultEvidence) -> String {
    match &tr.method {
        Some(method) => format!("{} (method: {method})", tr.result),
        None => tr.result.clone(),
    }
}

fn apply_flash_point(sds: &mut SdsRoot, mv: &MeasuredValueEvidence) {
    let pcp = sds
        .physical_chemical_properties
        .get_or_insert_with(Default::default);
    pcp.flash_point
        .get_or_insert_with(Vec::new)
        .push(PhysicalChemicalPropertiesFlashPoint {
            numeric_range_with_unit_and_qualifier: Some(numeric_exact(mv.value, &mv.unit)),
            method: mv.method.clone(),
            condition: format_conditions(&mv.conditions),
            additional_info: None,
        });
}

fn apply_boiling_point(sds: &mut SdsRoot, mv: &MeasuredValueEvidence) {
    let pcp = sds
        .physical_chemical_properties
        .get_or_insert_with(Default::default);
    pcp.boiling_point_related.get_or_insert_with(Vec::new).push(
        PhysicalChemicalPropertiesBoilingPointRelated {
            item_name: Some("Initial boiling point / boiling range".to_string()),
            numeric_range_with_unit_and_qualifier: Some(numeric_exact(mv.value, &mv.unit)),
            method: mv.method.clone(),
            condition: Some(PhysicalChemicalPropertiesBoilingPointRelatedCondition {
                pressure_value: mv.conditions.pressure_kpa,
                unit: mv.conditions.pressure_kpa.map(|_| "kPa".to_string()),
                other_condition: mv.conditions.atmosphere.clone(),
            }),
            additional_info: None,
        },
    );
}

fn apply_vapor_pressure(sds: &mut SdsRoot, mv: &MeasuredValueEvidence) {
    let pcp = sds
        .physical_chemical_properties
        .get_or_insert_with(Default::default);
    pcp.vapour_pressure.get_or_insert_with(Vec::new).push(
        PhysicalChemicalPropertiesVapourPressure {
            numeric_range_with_unit_and_qualifier: Some(numeric_exact(mv.value, &mv.unit)),
            method: mv.method.clone(),
            condition: Some(PhysicalChemicalPropertiesVapourPressureCondition {
                temperature: mv.conditions.temperature_c,
                unit: mv.conditions.temperature_c.map(|_| "°C".to_string()),
                other_condition: mv.conditions.atmosphere.clone(),
            }),
            additional_info: None,
        },
    );
}

fn apply_explosive_limits(sds: &mut SdsRoot, ev: &ExplosiveLimitsEvidence) {
    let pcp = sds
        .physical_chemical_properties
        .get_or_insert_with(Default::default);
    let condition = format_conditions(&ev.conditions);
    let list = pcp.explosion_limit.get_or_insert_with(Vec::new);
    if let Some(lower) = ev.lower {
        list.push(PhysicalChemicalPropertiesExplosionLimit {
            item_name: Some("Lower Explosive Limit".to_string()),
            numeric_range_with_unit_and_qualifier: Some(numeric_exact(lower, &ev.unit)),
            method: ev.method.clone(),
            condition: condition.clone(),
            additional_info: None,
        });
    }
    if let Some(upper) = ev.upper {
        list.push(PhysicalChemicalPropertiesExplosionLimit {
            item_name: Some("Upper Explosive Limit".to_string()),
            numeric_range_with_unit_and_qualifier: Some(numeric_exact(upper, &ev.unit)),
            method: ev.method.clone(),
            condition,
            additional_info: None,
        });
    }
}

fn apply_self_reactivity(sds: &mut SdsRoot, tr: &TestResultEvidence) {
    hazardous_reactions_mut(sds).self_reactivity_and_explosiveness = Some(describe_test_result(tr));
}

fn apply_oxidizing_properties(sds: &mut SdsRoot, tr: &TestResultEvidence) {
    hazardous_reactions_mut(sds).oxidizing_properties = Some(describe_test_result(tr));
}

fn hazardous_reactions_mut(sds: &mut SdsRoot) -> &mut StabilityReactivityHazardousReactions {
    sds.stability_reactivity
        .get_or_insert_with(Default::default)
        .hazardous_reactions
        .get_or_insert_with(Default::default)
}

fn apply_metal_corrosivity(sds: &mut SdsRoot, tr: &TestResultEvidence) {
    let effect = physicochemical_effect_mut(sds);
    effect.corrosive_to_metals = Some(describe_test_result(tr));
}

fn physicochemical_effect_mut(
    sds: &mut SdsRoot,
) -> &mut HazardIdentificationClassificationPhysicochemicalEffect {
    sds.hazard_identification
        .get_or_insert_with(Default::default)
        .classification
        .get_or_insert_with(Default::default)
        .physicochemical_effect
        .get_or_insert_with(Default::default)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generation::input::{ComponentInput, ConcentrationRange, SupplierInput};
    use crate::generation::EvidenceLevel;

    fn evidence(
        id: &str,
        level: EvidenceLevel,
        applies_to: EvidenceApplicability,
    ) -> EvidenceSource {
        EvidenceSource {
            id: id.to_string(),
            level,
            reference: format!("Report {id}"),
            issuer: None,
            document_date: None,
            applies_to,
        }
    }

    fn conditions(temperature_c: Option<f64>) -> MeasurementConditions {
        MeasurementConditions {
            temperature_c,
            pressure_kpa: None,
            atmosphere: None,
        }
    }

    fn measured(
        value: f64,
        unit: &str,
        evidence_id: &str,
        method: Option<&str>,
        temperature_c: Option<f64>,
    ) -> MeasuredValueEvidence {
        MeasuredValueEvidence {
            value,
            unit: unit.to_string(),
            method: method.map(str::to_string),
            conditions: conditions(temperature_c),
            sample_id: Some("S-1".to_string()),
            batch_id: Some("B-1".to_string()),
            evidence_id: evidence_id.to_string(),
        }
    }

    fn test_result(result: &str, evidence_id: &str, method: Option<&str>) -> TestResultEvidence {
        TestResultEvidence {
            result: result.to_string(),
            method: method.map(str::to_string),
            sample_id: None,
            batch_id: None,
            evidence_id: evidence_id.to_string(),
        }
    }

    fn base_product() -> ProductInput {
        ProductInput {
            trade_name: "Test Product".into(),
            other_names: vec![],
            supplier: SupplierInput {
                company_name: "Example Co.".into(),
                address: None,
                phone: None,
                email: None,
            },
            components: vec![ComponentInput {
                cas_number: Some("7732-18-5".into()),
                name: Some("Water".into()),
                concentration: ConcentrationRange {
                    exact: Some(100.0),
                    lower: None,
                    upper: None,
                    unit: "%".into(),
                },
            }],
            measured_properties: Default::default(),
            evidence: vec![],
        }
    }

    // --- flash point ---

    #[test]
    fn flash_point_with_eligible_product_test_evidence_becomes_confirmed() {
        let mut input = base_product();
        input.evidence.push(evidence(
            "ev1",
            EvidenceLevel::ProductTestReport,
            EvidenceApplicability::FinishedProduct,
        ));
        input.measured_properties.flash_point.push(measured(
            61.0,
            "°C",
            "ev1",
            Some("Closed Cup (ASTM D93)"),
            None,
        ));

        let mut sds = SdsRoot::default();
        let (unresolved, provenance) = resolve_measured_properties(&input, &mut sds);

        assert!(!unresolved.iter().any(|f| f.path == FLASH_POINT_PATH));
        assert!(provenance.iter().any(|p| p.path == FLASH_POINT_PATH));
        let fp = &sds
            .physical_chemical_properties
            .unwrap()
            .flash_point
            .unwrap()[0];
        assert_eq!(
            fp.numeric_range_with_unit_and_qualifier
                .as_ref()
                .unwrap()
                .exact_value
                .as_ref()
                .unwrap()
                .value,
            Some(61.0)
        );
        assert_eq!(fp.method.as_deref(), Some("Closed Cup (ASTM D93)"));
    }

    #[test]
    fn flash_point_value_with_unresolvable_evidence_id_stays_unresolved() {
        let mut input = base_product();
        // No matching EvidenceSource for "missing-ev" — evidence_id doesn't resolve.
        input.measured_properties.flash_point.push(measured(
            61.0,
            "°C",
            "missing-ev",
            Some("Closed Cup"),
            None,
        ));

        let mut sds = SdsRoot::default();
        let (unresolved, _) = resolve_measured_properties(&input, &mut sds);

        let field = unresolved
            .iter()
            .find(|f| f.path == FLASH_POINT_PATH)
            .unwrap();
        assert_eq!(field.reason, UnresolvedReason::MissingInput);
        assert!(sds.physical_chemical_properties.is_none());
    }

    #[test]
    fn flash_point_evidence_without_test_method_does_not_satisfy_policy() {
        let mut input = base_product();
        input.evidence.push(evidence(
            "ev1",
            EvidenceLevel::ProductTestReport,
            EvidenceApplicability::FinishedProduct,
        ));
        input
            .measured_properties
            .flash_point
            .push(measured(61.0, "°C", "ev1", None, None));

        let mut sds = SdsRoot::default();
        let (unresolved, _) = resolve_measured_properties(&input, &mut sds);

        let field = unresolved
            .iter()
            .find(|f| f.path == FLASH_POINT_PATH)
            .unwrap();
        assert_eq!(
            field.reason,
            UnresolvedReason::InsufficientMeasurementConditions
        );
        assert!(sds.physical_chemical_properties.is_none());
    }

    // --- vapor pressure ---

    #[test]
    fn vapor_pressure_without_measurement_temperature_remains_unresolved() {
        let mut input = base_product();
        input.evidence.push(evidence(
            "ev1",
            EvidenceLevel::ProductTestReport,
            EvidenceApplicability::FinishedProduct,
        ));
        input.measured_properties.vapor_pressure.push(measured(
            2.3,
            "kPa",
            "ev1",
            Some("Static method"),
            None,
        ));

        let mut sds = SdsRoot::default();
        let (unresolved, _) = resolve_measured_properties(&input, &mut sds);

        let field = unresolved
            .iter()
            .find(|f| f.path == VAPOR_PRESSURE_PATH)
            .unwrap();
        assert_eq!(
            field.reason,
            UnresolvedReason::InsufficientMeasurementConditions
        );
    }

    #[test]
    fn vapor_pressure_with_complete_eligible_evidence_resolves() {
        let mut input = base_product();
        input.evidence.push(evidence(
            "ev1",
            EvidenceLevel::ProductTestReport,
            EvidenceApplicability::FinishedProduct,
        ));
        input.measured_properties.vapor_pressure.push(measured(
            2.3,
            "kPa",
            "ev1",
            Some("Static method"),
            Some(20.0),
        ));

        let mut sds = SdsRoot::default();
        let (unresolved, provenance) = resolve_measured_properties(&input, &mut sds);

        assert!(!unresolved.iter().any(|f| f.path == VAPOR_PRESSURE_PATH));
        let prov = provenance
            .iter()
            .find(|p| p.path == VAPOR_PRESSURE_PATH)
            .unwrap();
        assert_eq!(prov.conditions.as_ref().unwrap().temperature_c, Some(20.0));
        let vp = &sds
            .physical_chemical_properties
            .unwrap()
            .vapour_pressure
            .unwrap()[0];
        assert_eq!(vp.condition.as_ref().unwrap().temperature, Some(20.0));
    }

    // --- applicability / evidence eligibility ---

    #[test]
    fn component_level_evidence_does_not_confirm_a_mixture_property() {
        let mut input = base_product();
        input.evidence.push(evidence(
            "ev1",
            EvidenceLevel::ProductTestReport,
            EvidenceApplicability::Component,
        ));
        input.measured_properties.flash_point.push(measured(
            61.0,
            "°C",
            "ev1",
            Some("Closed Cup"),
            None,
        ));

        let mut sds = SdsRoot::default();
        let (unresolved, _) = resolve_measured_properties(&input, &mut sds);

        let field = unresolved
            .iter()
            .find(|f| f.path == FLASH_POINT_PATH)
            .unwrap();
        assert_eq!(
            field.reason,
            UnresolvedReason::MixtureCannotBeDerivedFromComponents
        );
    }

    #[test]
    fn equivalent_batch_evidence_is_accepted_under_explicit_policy() {
        let mut input = base_product();
        input.evidence.push(evidence(
            "ev1",
            EvidenceLevel::EquivalentBatchTestReport,
            EvidenceApplicability::EquivalentBatch,
        ));
        input.measured_properties.flash_point.push(measured(
            61.0,
            "°C",
            "ev1",
            Some("Closed Cup"),
            None,
        ));

        let mut sds = SdsRoot::default();
        let (unresolved, _) = resolve_measured_properties(&input, &mut sds);

        assert!(!unresolved.iter().any(|f| f.path == FLASH_POINT_PATH));
    }

    #[test]
    fn conflicting_reports_produce_conflicting_sources_and_block_release() {
        let mut input = base_product();
        input.evidence.push(evidence(
            "ev1",
            EvidenceLevel::ProductTestReport,
            EvidenceApplicability::FinishedProduct,
        ));
        input.evidence.push(evidence(
            "ev2",
            EvidenceLevel::ProductTestReport,
            EvidenceApplicability::FinishedProduct,
        ));
        input.measured_properties.flash_point.push(measured(
            61.0,
            "°C",
            "ev1",
            Some("Closed Cup"),
            None,
        ));
        input.measured_properties.flash_point.push(measured(
            65.0,
            "°C",
            "ev2",
            Some("Closed Cup"),
            None,
        ));

        let mut sds = SdsRoot::default();
        let (unresolved, _) = resolve_measured_properties(&input, &mut sds);

        let field = unresolved
            .iter()
            .find(|f| f.path == FLASH_POINT_PATH)
            .unwrap();
        assert_eq!(field.reason, UnresolvedReason::ConflictingSources);
        assert!(field.blocks_release);
        assert!(sds.physical_chemical_properties.is_none());
    }

    #[test]
    fn ph_grade_evidence_does_not_resolve_metal_corrosivity() {
        // No pH field exists in this schema to accidentally accept — the
        // closest analog is evidence of a level the field's policy doesn't
        // list (only ProductTestReport/EquivalentBatchTestReport are
        // eligible for metal corrosivity).
        let mut input = base_product();
        input.evidence.push(evidence(
            "ev1",
            EvidenceLevel::SupplierSpecification,
            EvidenceApplicability::FinishedProduct,
        ));
        input
            .measured_properties
            .metal_corrosivity
            .push(test_result("pH 6.5", "ev1", Some("pH meter")));

        let mut sds = SdsRoot::default();
        let (unresolved, _) = resolve_measured_properties(&input, &mut sds);

        let field = unresolved
            .iter()
            .find(|f| f.path == METAL_CORROSIVITY_PATH)
            .unwrap();
        assert_eq!(field.reason, UnresolvedReason::ProductTestRequired);
        assert!(sds.hazard_identification.is_none());
    }

    #[test]
    fn dsc_only_evidence_does_not_resolve_self_reactivity() {
        let mut input = base_product();
        // DSC screening modeled as SupplierSpecification-grade — not in
        // self-reactivity's allowed_evidence (ProductTestReport /
        // EquivalentBatchTestReport only), matching the architecture
        // doc's "DSC screening alone is not sufficient" rule.
        input.evidence.push(evidence(
            "ev1",
            EvidenceLevel::SupplierSpecification,
            EvidenceApplicability::FinishedProduct,
        ));
        input.measured_properties.self_reactivity.push(test_result(
            "DSC exotherm onset 180°C",
            "ev1",
            Some("DSC"),
        ));

        let mut sds = SdsRoot::default();
        let (unresolved, _) = resolve_measured_properties(&input, &mut sds);

        let field = unresolved
            .iter()
            .find(|f| f.path == SELF_REACTIVITY_PATH)
            .unwrap();
        assert_eq!(field.reason, UnresolvedReason::ProductTestRequired);
    }

    #[test]
    fn structural_or_reference_evidence_does_not_resolve_oxidizing_properties() {
        let mut input = base_product();
        input.evidence.push(evidence(
            "ev1",
            EvidenceLevel::ReferenceDatabase,
            EvidenceApplicability::ReferenceSubstance,
        ));
        input
            .measured_properties
            .oxidizing_properties
            .push(test_result(
                "structural alert: peroxide-forming",
                "ev1",
                None,
            ));

        let mut sds = SdsRoot::default();
        let (unresolved, _) = resolve_measured_properties(&input, &mut sds);

        let field = unresolved
            .iter()
            .find(|f| f.path == OXIDIZING_PROPERTIES_PATH)
            .unwrap();
        assert_eq!(field.reason, UnresolvedReason::ProductTestRequired);
    }

    // --- provenance content ---

    #[test]
    fn provenance_includes_evidence_reference_method_batch_sample_and_conditions() {
        let mut input = base_product();
        input.evidence.push(evidence(
            "ev1",
            EvidenceLevel::ProductTestReport,
            EvidenceApplicability::FinishedProduct,
        ));
        input.measured_properties.flash_point.push(measured(
            61.0,
            "°C",
            "ev1",
            Some("Closed Cup"),
            None,
        ));

        let mut sds = SdsRoot::default();
        let (_, provenance) = resolve_measured_properties(&input, &mut sds);

        let p = provenance
            .iter()
            .find(|p| p.path == FLASH_POINT_PATH)
            .unwrap();
        assert_eq!(p.source_reference.as_deref(), Some("Report ev1"));
        assert_eq!(p.test_method.as_deref(), Some("Closed Cup"));
        assert_eq!(p.sample_id.as_deref(), Some("S-1"));
        assert_eq!(p.batch_id.as_deref(), Some("B-1"));
        assert!(p.conditions.is_some());
        assert_eq!(p.source_type, EvidenceLevel::ProductTestReport);
        assert_eq!(p.confidence, crate::generation::ConfidenceLevel::High);
    }

    // --- official JSON separation ---

    #[test]
    fn evidence_metadata_does_not_leak_into_official_sds_json() {
        let mut input = base_product();
        input.evidence.push(evidence(
            "ev1",
            EvidenceLevel::ProductTestReport,
            EvidenceApplicability::FinishedProduct,
        ));
        input.measured_properties.flash_point.push(measured(
            61.0,
            "°C",
            "ev1",
            Some("Closed Cup"),
            None,
        ));

        let mut sds = SdsRoot::default();
        resolve_measured_properties(&input, &mut sds);

        let json = serde_json::to_string(&sds).unwrap();
        for leak in [
            "evidence_id",
            "confidence",
            "ProductTestReport",
            "Report ev1",
            "S-1",
            "B-1",
        ] {
            assert!(
                !json.contains(leak),
                "official SDS JSON must not contain '{leak}'"
            );
        }
    }
}
