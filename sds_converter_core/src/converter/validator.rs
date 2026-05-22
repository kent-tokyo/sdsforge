use crate::schema::SdsRoot;

/// Performs post-deserialization structural checks on all 16 SDS sections.
/// Does not hard-fail so partial results are still usable.
pub fn validate(sds: &SdsRoot) -> Vec<String> {
    let mut w: Vec<String> = Vec::new();

    macro_rules! missing {
        ($section:literal) => {
            w.push(format!(
                "{}: section not extracted — check source document.",
                $section
            ))
        };
    }

    // ── Section 1: Identification ─────────────────────────────────────────────
    match &sds.identification {
        None => missing!("Section 1 (Identification)"),
        Some(id) => {
            let has_name = id
                .trade_product_identity
                .as_ref()
                .map(|t| t.trade_name_jp.is_some() || t.trade_name_en.is_some())
                .unwrap_or(false);
            if !has_name {
                w.push("Section 1 (Identification): no product name (TradeNameJP/TradeNameEN).".into());
            }
            if id.supplier_information.is_none() {
                w.push("Section 1 (Identification): SupplierInformation is missing.".into());
            }
        }
    }

    // ── Section 2: HazardIdentification ──────────────────────────────────────
    match &sds.hazard_identification {
        None => missing!("Section 2 (HazardIdentification)"),
        Some(hz) => {
            if hz.classification.is_none() && hz.hazard_labelling.is_none() {
                w.push("Section 2 (HazardIdentification): neither Classification nor HazardLabelling extracted.".into());
            }
        }
    }

    // ── Section 3: Composition ────────────────────────────────────────────────
    match &sds.composition {
        None => missing!("Section 3 (Composition)"),
        Some(comp) => {
            let empty = comp
                .composition_and_concentration
                .as_ref()
                .map(|v| v.is_empty())
                .unwrap_or(true);
            if empty {
                w.push("Section 3 (Composition): CompositionAndConcentration is empty.".into());
            }
        }
    }

    // ── Section 4: FirstAidMeasures ───────────────────────────────────────────
    if sds.first_aid_measures.is_none() {
        missing!("Section 4 (FirstAidMeasures)");
    }

    // ── Section 5: FireFightingMeasures ───────────────────────────────────────
    if sds.fire_fighting_measures.is_none() {
        missing!("Section 5 (FireFightingMeasures)");
    }

    // ── Section 6: AccidentalReleaseMeasures ─────────────────────────────────
    if sds.accidental_release_measures.is_none() {
        missing!("Section 6 (AccidentalReleaseMeasures)");
    }

    // ── Section 7: HandlingAndStorage ────────────────────────────────────────
    if sds.handling_and_storage.is_none() {
        missing!("Section 7 (HandlingAndStorage)");
    }

    // ── Section 8: ExposureControlPersonalProtection ─────────────────────────
    if sds.exposure_control_personal_protection.is_none() {
        missing!("Section 8 (ExposureControlPersonalProtection)");
    }

    // ── Section 9: PhysicalChemicalProperties ────────────────────────────────
    if sds.physical_chemical_properties.is_none() {
        missing!("Section 9 (PhysicalChemicalProperties)");
    }

    // ── Section 10: StabilityReactivity ──────────────────────────────────────
    match &sds.stability_reactivity {
        None => missing!("Section 10 (StabilityReactivity)"),
        Some(sr) => {
            if sr.stability_description.is_none() && sr.reactivity_description.is_none() {
                w.push("Section 10 (StabilityReactivity): neither StabilityDescription nor ReactivityDescription extracted.".into());
            }
        }
    }

    // ── Section 11: ToxicologicalInformation ─────────────────────────────────
    match &sds.toxicological_information {
        None => missing!("Section 11 (ToxicologicalInformation)"),
        Some(list) if list.is_empty() => {
            w.push("Section 11 (ToxicologicalInformation): array is present but empty.".into());
        }
        _ => {}
    }

    // ── Section 12: EcologicalInformation ────────────────────────────────────
    match &sds.ecological_information {
        None => missing!("Section 12 (EcologicalInformation)"),
        Some(list) if list.is_empty() => {
            w.push("Section 12 (EcologicalInformation): array is present but empty.".into());
        }
        _ => {}
    }

    // ── Section 13: DisposalConsiderations ───────────────────────────────────
    if sds.disposal_considerations.is_none() {
        missing!("Section 13 (DisposalConsiderations)");
    }

    // ── Section 14: TransportInformation ─────────────────────────────────────
    if sds.transport_information.is_none() {
        missing!("Section 14 (TransportInformation)");
    }

    // ── Section 15: RegulatoryInformation ────────────────────────────────────
    if sds.regulatory_information.is_none() {
        missing!("Section 15 (RegulatoryInformation)");
    }

    // ── Section 16: OtherInformation ─────────────────────────────────────────
    if sds.other_information.is_none() {
        missing!("Section 16 (OtherInformation)");
    }

    w
}
