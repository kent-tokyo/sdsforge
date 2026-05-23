use crate::ghs_codes;
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
            if let Some(hl) = &hz.hazard_labelling {
                // GHS H-code validation
                if let Some(stmts) = &hl.hazard_statement {
                    for s in stmts {
                        if let Some(code) = &s.hazard_statement_code {
                            let upper = code.to_uppercase();
                            if !ghs_codes::is_valid_h_code(&upper) {
                                w.push(format!(
                                    "Section 2 (HazardStatement): unknown H-code '{code}'"
                                ));
                            }
                        }
                    }
                }
                // GHS P-code validation
                if let Some(ps) = &hl.precautionary_statements {
                    let all_codes: Vec<Option<&String>> = ps
                        .prevention
                        .iter()
                        .flatten()
                        .map(|s| s.precautionary_statement_code.as_ref())
                        .chain(
                            ps.response
                                .iter()
                                .flatten()
                                .map(|s| s.precautionary_statement_code.as_ref()),
                        )
                        .chain(
                            ps.storage
                                .iter()
                                .flatten()
                                .map(|s| s.precautionary_statement_code.as_ref()),
                        )
                        .chain(
                            ps.disposal
                                .iter()
                                .flatten()
                                .map(|s| s.precautionary_statement_code.as_ref()),
                        )
                        .collect();
                    for code in all_codes.into_iter().flatten() {
                        let upper = code.to_uppercase();
                        if !ghs_codes::is_valid_p_code(&upper) {
                            w.push(format!(
                                "Section 2 (PrecautionaryStatement): unknown P-code '{code}'"
                            ));
                        }
                    }
                }
            }
        }
    }

    // ── Section 3: Composition ────────────────────────────────────────────────
    match &sds.composition {
        None => missing!("Section 3 (Composition)"),
        Some(comp) => {
            let items = comp.composition_and_concentration.as_deref().unwrap_or(&[]);
            if items.is_empty() {
                w.push("Section 3 (Composition): CompositionAndConcentration is empty.".into());
            }
            // CAS number format validation
            for (i, item) in items.iter().enumerate() {
                if let Some(ids) = &item.substance_identifiers {
                    if let Some(identity) = &ids.substance_identity {
                        if let Some(cas_node) = &identity.ca_sno {
                            for cas in cas_node.full_text.iter().flatten() {
                                if !validate_cas_format(cas) {
                                    w.push(format!(
                                        "Section 3 (Composition[{i}]): invalid CAS format '{cas}'"
                                    ));
                                }
                            }
                        }
                    }
                }
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

/// Validate a CAS Registry Number.
///
/// Format: `^\d{2,7}-\d{2}-\d$`
/// Check digit: weighted sum of all digits (right to left, weight starts at 1 for the
/// rightmost non-check digit) mod 10 must equal the check digit.
pub(crate) fn validate_cas_format(cas: &str) -> bool {
    let parts: Vec<&str> = cas.split('-').collect();
    if parts.len() != 3 {
        return false;
    }
    let (a, b, c) = (parts[0], parts[1], parts[2]);
    // Format check
    if a.len() < 2
        || a.len() > 7
        || b.len() != 2
        || c.len() != 1
        || !a.chars().all(|c| c.is_ascii_digit())
        || !b.chars().all(|c| c.is_ascii_digit())
        || !c.chars().all(|c| c.is_ascii_digit())
    {
        return false;
    }
    let check_digit: u32 = c.chars().next().unwrap().to_digit(10).unwrap();
    // Build digit string excluding check digit, then compute weighted sum
    let digits: Vec<u32> = a
        .chars()
        .chain(b.chars())
        .filter_map(|ch| ch.to_digit(10))
        .collect();
    let sum: u32 = digits
        .iter()
        .rev()
        .enumerate()
        .map(|(i, &d)| d * (i as u32 + 1))
        .sum();
    sum % 10 == check_digit
}
