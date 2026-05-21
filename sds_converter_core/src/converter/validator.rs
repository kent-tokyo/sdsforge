use tracing::warn;

use crate::schema::SdsRoot;

/// Performs post-deserialization structural checks.
/// Logs warnings for suspicious patterns; does not hard-fail so partial results are still usable.
pub fn validate(sds: &SdsRoot) -> Vec<String> {
    let mut warnings = Vec::new();

    // Check that at least some content was extracted
    let has_identification = sds.identification.is_some();
    let has_any_section = sds.hazard_identification.is_some()
        || sds.composition.is_some()
        || sds.physical_chemical_properties.is_some()
        || sds.toxicological_information.is_some();

    if !has_identification && !has_any_section {
        warnings.push(
            "No SDS sections were extracted. The document may be scanned, image-based, or empty."
                .to_string(),
        );
    }

    // Check product name
    if let Some(id) = &sds.identification {
        let has_name = id
            .trade_product_identity
            .as_ref()
            .map(|t| t.trade_name_jp.is_some() || t.trade_name_en.is_some())
            .unwrap_or(false);
        if !has_name {
            warnings.push("Section 1 (Identification): no product name (TradeNameJP/TradeNameEN) found.".to_string());
        }
        if id.supplier_information.is_none() {
            warnings.push("Section 1 (Identification): SupplierInformation is missing.".to_string());
        }
    }

    // Check hazard section
    if let Some(hz) = &sds.hazard_identification {
        if hz.classification.is_none() && hz.hazard_labelling.is_none() {
            warnings.push("Section 2 (HazardIdentification): neither Classification nor HazardLabelling was extracted.".to_string());
        }
    }

    // Warn if ToxicologicalInformation array is unexpectedly empty
    if let Some(tox_list) = &sds.toxicological_information {
        if tox_list.is_empty() {
            warnings.push("Section 11 (ToxicologicalInformation): array is present but empty.".to_string());
        }
    }

    // Warn if EcologicalInformation array is unexpectedly empty
    if let Some(eco_list) = &sds.ecological_information {
        if eco_list.is_empty() {
            warnings.push("Section 12 (EcologicalInformation): array is present but empty.".to_string());
        }
    }

    for w in &warnings {
        warn!("{w}");
    }

    warnings
}
