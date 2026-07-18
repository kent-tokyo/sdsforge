use std::collections::HashMap;

use crate::converter::validator::{validate_cas_format, Finding};
use crate::enrichment::{lookup_cas, CasInfo};
use crate::schema::{
    Composition, CompositionCompositionAndConcentration,
    CompositionCompositionAndConcentrationConcentration, Identification,
    IdentificationSupplierInformation, IdentificationTradeProductIdentity,
    NumericRangeWithUnitAndQualifier, NumericRangeWithUnitAndQualifierExactValue,
    NumericRangeWithUnitAndQualifierLowerValue, NumericRangeWithUnitAndQualifierUpperValue,
    SdsRoot, SubstanceIdentifiers, SubstanceIdentifiersSubstanceIdentity,
    SubstanceIdentifiersSubstanceIdentityCASno, SubstanceIdentifiersSubstanceNames,
};

use super::input::{ConcentrationRange, ProductInput};

/// A partial SDS draft covering **only** Section 1 (Identification) and
/// Section 3 (Composition) — the sections derivable from supplied product/
/// supplier/CAS/concentration data without product testing. Every other
/// `SdsRoot` field is left `None`.
///
/// Interim type: roadmap commit #10 introduces `GenerationResult`, which
/// carries additional fields (`unresolved`, `provenance`, `evidence_summary`,
/// `release_status`) that don't exist yet. `SectionDraftResult` is expected
/// to be wrapped or replaced by `GenerationResult` at that point, not
/// extended in place — keep it minimal.
#[derive(Debug, Clone)]
pub struct SectionDraftResult {
    pub sds: SdsRoot,
    pub findings: Vec<Finding>,
}

/// Pure, deterministic mapping from [`ProductInput`] to a Section 1/3 draft.
///
/// Performs no I/O. `resolved` supplies CAS lookup results the caller has
/// already fetched (e.g. via [`generate_section_1_and_3`]) — a component
/// whose CAS is absent from this map is treated as "enrichment unavailable"
/// for that component, not as an error; its supplied data (name, CAS,
/// concentration, unit) is still mapped into the draft. This split exists
/// so mapping logic can be tested without PubChem availability: pass an
/// empty or partial map to exercise the lookup-failure path deterministically.
///
/// Does not re-validate CAS format/check-digit or concentration sanity —
/// that's roadmap commit #8's `validate_product_input`, already run
/// separately. Its findings remain authoritative; this function does not
/// silently repair invalid input, it maps whatever was supplied.
pub fn draft_sections_from_resolved_input(
    input: &ProductInput,
    resolved: &HashMap<String, CasInfo>,
) -> SectionDraftResult {
    let mut findings = Vec::new();

    let identification = Identification {
        trade_product_identity: Some(IdentificationTradeProductIdentity {
            // The schema splits trade names by language (TradeNameJP/EN);
            // ProductInput carries a single untagged `trade_name`. Mapping
            // it to the JP slot (this schema's primary/first name field,
            // matching the project's Japan-first default elsewhere) rather
            // than guessing a language or duplicating it into both fields —
            // duplicating into TradeNameEN would assert an English name we
            // were never actually given.
            trade_name_jp: Some(input.trade_name.clone()),
            trade_name_en: None,
            other_name: non_empty(input.other_names.clone()),
            product_no_user: None,
            additional_info: None,
        }),
        specification_no: None,
        supplier_information: Some(IdentificationSupplierInformation {
            company_name: Some(input.supplier.company_name.clone()),
            department: None,
            name: None,
            post_code: None,
            address: input.supplier.address.clone(),
            phone: input.supplier.phone.clone(),
            working_hours: None,
            fax: None,
            email: input.supplier.email.clone(),
            company_url: None,
            // SupplierInput has one general `phone`, not a distinct
            // emergency number — populating EmergencyContact from it would
            // be inferring an emergency contact from an unrelated field.
            emergency_contact: None,
            additional_info: None,
        }),
        use_and_use_advised_against: None,
        domestic_manufacturer_information: None,
        additional_info: None,
    };

    let rows: Vec<CompositionCompositionAndConcentration> = input
        .components
        .iter()
        .map(|component| {
            let cas_info = component
                .cas_number
                .as_ref()
                .and_then(|cas| resolved.get(cas));

            if let Some(cas) = &component.cas_number {
                if cas_info.is_none() && validate_cas_format(cas) {
                    findings.push(Finding {
                        level: "LOW".into(),
                        rule: "GEN-CAS-ENRICHMENT-MISSING".into(),
                        message: format!(
                            "CAS '{cas}': no enrichment data available (not found, or lookup not performed). Supplied data was kept as-is."
                        ),
                    });
                }
            }

            component_to_row(component, cas_info)
        })
        .collect();

    let composition = Composition {
        composition_type: None,
        substance_identifiers: None,
        r#use: None,
        composition_and_concentration: non_empty(rows),
        impurities_and_stabilizing_additives: None,
        additional_info: None,
    };

    let sds = SdsRoot {
        identification: Some(identification),
        composition: Some(composition),
        ..Default::default()
    };

    SectionDraftResult { sds, findings }
}

fn component_to_row(
    component: &super::input::ComponentInput,
    cas_info: Option<&CasInfo>,
) -> CompositionCompositionAndConcentration {
    let cas_identity =
        component
            .cas_number
            .as_ref()
            .map(|cas| SubstanceIdentifiersSubstanceIdentity {
                ca_sno: Some(SubstanceIdentifiersSubstanceIdentityCASno {
                    full_text: Some(vec![cas.clone()]),
                    additional_info: None,
                }),
                other_no: None,
            });

    let substance_identifiers =
        if component.name.is_some() || cas_identity.is_some() || cas_info.is_some() {
            Some(SubstanceIdentifiers {
                substance_names: Some(SubstanceIdentifiersSubstanceNames {
                    // GenericName carries the name the caller supplied.
                    // IupacName carries the name the existing CAS resolver
                    // returned — a deterministic 1:1 lookup by CAS number, not
                    // a choice among ambiguous candidates, so populating it
                    // here isn't the kind of identity-guessing this feature
                    // avoids elsewhere.
                    iupac_name: cas_info.and_then(|info| info.iupac_name.clone()),
                    cas_inventory_name: None,
                    generic_name: component.name.clone(),
                }),
                common_name: None,
                substance_identity: cas_identity,
                cbi: None,
                additional_info: None,
            })
        } else {
            None
        };

    CompositionCompositionAndConcentration {
        substance_identifiers,
        molecular_formula: cas_info.and_then(|info| info.molecular_formula.clone()),
        structural_formula: None,
        structural_formula_path_and_file_name: None,
        smiles: None,
        in_ch_i: None,
        in_ch_i_key: None,
        molecular_weight: None,
        concentration: Some(CompositionCompositionAndConcentrationConcentration {
            numeric_range_with_unit_and_qualifier: Some(concentration_range_to_schema(
                &component.concentration,
            )),
        }),
        gazette_no: None,
        r#use: None,
        gh_sinfo: None,
        additional_info: None,
    }
}

/// Maps [`ConcentrationRange`] preserving exact-vs-range meaning — an exact
/// value stays exact, a lower/upper pair stays a range. Never converted to a
/// midpoint, and an exact value is never serialized as an artificial
/// lower==upper range (the schema doesn't require that representation).
fn concentration_range_to_schema(range: &ConcentrationRange) -> NumericRangeWithUnitAndQualifier {
    NumericRangeWithUnitAndQualifier {
        exact_value: range
            .exact
            .map(|v| NumericRangeWithUnitAndQualifierExactValue {
                value_symbol: None,
                value: Some(v),
            }),
        lower_value: range
            .lower
            .map(|v| NumericRangeWithUnitAndQualifierLowerValue {
                value_symbol: None,
                value: Some(v),
            }),
        upper_value: range
            .upper
            .map(|v| NumericRangeWithUnitAndQualifierUpperValue {
                value_symbol: None,
                value: Some(v),
            }),
        unit: non_empty_string(range.unit.clone()),
        additional_info: None,
    }
}

fn non_empty<T>(v: Vec<T>) -> Option<Vec<T>> {
    if v.is_empty() {
        None
    } else {
        Some(v)
    }
}

fn non_empty_string(s: String) -> Option<String> {
    if s.trim().is_empty() {
        None
    } else {
        Some(s)
    }
}

/// Orchestration: performs CAS enrichment lookups (network I/O against
/// PubChem via [`crate::enrichment::lookup_cas`] — no second PubChem client
/// or CAS resolver is implemented here), then delegates all mapping and
/// finding logic to [`draft_sections_from_resolved_input`].
///
/// A lookup failure for one component never discards that component's
/// supplied data — it's still mapped, with a `Finding` noting enrichment
/// was unavailable. Guessing a chemical identity from a failed lookup would
/// produce a more dangerous draft than an incomplete-but-truthful one.
pub async fn generate_section_1_and_3(
    input: &ProductInput,
    client: &reqwest::Client,
) -> SectionDraftResult {
    let mut resolved = HashMap::new();
    for component in &input.components {
        if let Some(cas) = &component.cas_number {
            if let Ok(Some(info)) = lookup_cas(cas, client).await {
                resolved.insert(cas.clone(), info);
            }
        }
    }
    draft_sections_from_resolved_input(input, &resolved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generation::input::{ComponentInput, SupplierInput};
    use crate::generation::validate_product_input;

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

    fn range_component(cas: &str, name: &str, lower: f64, upper: f64) -> ComponentInput {
        ComponentInput {
            cas_number: Some(cas.into()),
            name: Some(name.into()),
            concentration: ConcentrationRange {
                exact: None,
                lower: Some(lower),
                upper: Some(upper),
                unit: "%".into(),
            },
        }
    }

    #[test]
    fn single_component_maps_section_1_and_3() {
        let input = ProductInput {
            trade_name: "Test Solvent".into(),
            other_names: vec![],
            supplier: supplier(),
            components: vec![exact_component("7732-18-5", "Water", 100.0)],
            measured_properties: Default::default(),
            evidence: vec![],
        };
        let result = draft_sections_from_resolved_input(&input, &HashMap::new());

        let ident = result.sds.identification.as_ref().unwrap();
        let identity = ident.trade_product_identity.as_ref().unwrap();
        assert_eq!(identity.trade_name_jp.as_deref(), Some("Test Solvent"));

        let supplier_info = ident.supplier_information.as_ref().unwrap();
        assert_eq!(
            supplier_info.company_name.as_deref(),
            Some("Example Chemical Co.")
        );
        assert_eq!(supplier_info.phone.as_deref(), Some("03-1234-5678"));

        let rows = result
            .sds
            .composition
            .as_ref()
            .unwrap()
            .composition_and_concentration
            .as_ref()
            .unwrap();
        assert_eq!(rows.len(), 1);
        let cas = rows[0]
            .substance_identifiers
            .as_ref()
            .unwrap()
            .substance_identity
            .as_ref()
            .unwrap()
            .ca_sno
            .as_ref()
            .unwrap()
            .full_text
            .as_ref()
            .unwrap();
        assert_eq!(cas, &vec!["7732-18-5".to_string()]);

        let exact = rows[0]
            .concentration
            .as_ref()
            .unwrap()
            .numeric_range_with_unit_and_qualifier
            .as_ref()
            .unwrap()
            .exact_value
            .as_ref()
            .unwrap()
            .value;
        assert_eq!(exact, Some(100.0));
    }

    #[test]
    fn multi_component_preserves_order_and_range_shape() {
        let input = ProductInput {
            trade_name: "Test Mixture".into(),
            other_names: vec![],
            supplier: supplier(),
            components: vec![
                exact_component("7732-18-5", "Water", 60.0),
                range_component("64-17-5", "Ethanol", 30.0, 40.0),
            ],
            measured_properties: Default::default(),
            evidence: vec![],
        };
        let result = draft_sections_from_resolved_input(&input, &HashMap::new());
        let rows = result
            .sds
            .composition
            .as_ref()
            .unwrap()
            .composition_and_concentration
            .as_ref()
            .unwrap();
        assert_eq!(rows.len(), 2);

        // Order preserved: row 0 is Water, row 1 is Ethanol.
        let name0 = rows[0]
            .substance_identifiers
            .as_ref()
            .unwrap()
            .substance_names
            .as_ref()
            .unwrap()
            .generic_name
            .as_deref();
        assert_eq!(name0, Some("Water"));
        let name1 = rows[1]
            .substance_identifiers
            .as_ref()
            .unwrap()
            .substance_names
            .as_ref()
            .unwrap()
            .generic_name
            .as_deref();
        assert_eq!(name1, Some("Ethanol"));

        // Row 1's concentration is still a range, not collapsed to a midpoint.
        let conc1 = rows[1]
            .concentration
            .as_ref()
            .unwrap()
            .numeric_range_with_unit_and_qualifier
            .as_ref()
            .unwrap();
        assert!(conc1.exact_value.is_none());
        assert_eq!(conc1.lower_value.as_ref().unwrap().value, Some(30.0));
        assert_eq!(conc1.upper_value.as_ref().unwrap().value, Some(40.0));
    }

    #[test]
    fn product_identity_is_not_replaced_by_component_name() {
        let input = ProductInput {
            trade_name: "Brand X Cleaner".into(),
            other_names: vec![],
            supplier: supplier(),
            components: vec![exact_component("64-17-5", "Ethanol", 100.0)],
            measured_properties: Default::default(),
            evidence: vec![],
        };
        let result = draft_sections_from_resolved_input(&input, &HashMap::new());
        let trade_name = result
            .sds
            .identification
            .as_ref()
            .unwrap()
            .trade_product_identity
            .as_ref()
            .unwrap()
            .trade_name_jp
            .as_deref();
        assert_eq!(trade_name, Some("Brand X Cleaner"));
        assert_ne!(trade_name, Some("Ethanol"));
    }

    #[test]
    fn missing_optional_section1_data_is_not_fabricated() {
        let input = ProductInput {
            trade_name: "Minimal Product".into(),
            other_names: vec![],
            supplier: SupplierInput {
                company_name: "Minimal Co.".into(),
                address: None,
                phone: None,
                email: None,
            },
            components: vec![exact_component("7732-18-5", "Water", 100.0)],
            measured_properties: Default::default(),
            evidence: vec![],
        };
        let result = draft_sections_from_resolved_input(&input, &HashMap::new());
        let supplier_info = result
            .sds
            .identification
            .as_ref()
            .unwrap()
            .supplier_information
            .as_ref()
            .unwrap();
        assert!(supplier_info.address.is_none());
        assert!(supplier_info.phone.is_none());
        assert!(supplier_info.email.is_none());
        assert!(supplier_info.emergency_contact.is_none());
        assert!(result
            .sds
            .identification
            .as_ref()
            .unwrap()
            .domestic_manufacturer_information
            .is_none());
    }

    #[test]
    fn resolved_enrichment_is_mapped() {
        let input = ProductInput {
            trade_name: "Test".into(),
            other_names: vec![],
            supplier: supplier(),
            components: vec![exact_component("7732-18-5", "Water", 100.0)],
            measured_properties: Default::default(),
            evidence: vec![],
        };
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
        let result = draft_sections_from_resolved_input(&input, &resolved);
        let row = &result
            .sds
            .composition
            .as_ref()
            .unwrap()
            .composition_and_concentration
            .as_ref()
            .unwrap()[0];
        assert_eq!(row.molecular_formula.as_deref(), Some("H2O"));
        assert_eq!(
            row.substance_identifiers
                .as_ref()
                .unwrap()
                .substance_names
                .as_ref()
                .unwrap()
                .iupac_name
                .as_deref(),
            Some("oxidane")
        );
        // No lookup-failure finding when enrichment succeeded.
        assert!(!result
            .findings
            .iter()
            .any(|f| f.rule == "GEN-CAS-ENRICHMENT-MISSING"));
    }

    #[test]
    fn lookup_failure_keeps_supplied_data_and_returns_finding() {
        let input = ProductInput {
            trade_name: "Test".into(),
            other_names: vec![],
            supplier: supplier(),
            components: vec![exact_component("7732-18-5", "Water", 100.0)],
            measured_properties: Default::default(),
            evidence: vec![],
        };
        // Empty resolved map == lookup failed/unavailable for this CAS.
        let result = draft_sections_from_resolved_input(&input, &HashMap::new());

        let row = &result
            .sds
            .composition
            .as_ref()
            .unwrap()
            .composition_and_concentration
            .as_ref()
            .unwrap()[0];
        // Supplied data is preserved even though enrichment failed.
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
        assert!(row.molecular_formula.is_none());
        // No guessed IUPAC name inserted.
        assert!(row
            .substance_identifiers
            .as_ref()
            .unwrap()
            .substance_names
            .as_ref()
            .unwrap()
            .iupac_name
            .is_none());
        assert!(result
            .findings
            .iter()
            .any(|f| f.rule == "GEN-CAS-ENRICHMENT-MISSING"));
    }

    #[test]
    fn duplicate_cas_is_still_validated_and_not_merged() {
        let input = ProductInput {
            trade_name: "Test".into(),
            other_names: vec![],
            supplier: supplier(),
            components: vec![
                exact_component("7732-18-5", "Water", 50.0),
                exact_component("7732-18-5", "Water again", 50.0),
            ],
            measured_properties: Default::default(),
            evidence: vec![],
        };
        // Commit #8's validator still flags the duplicate independently.
        let input_findings = validate_product_input(&input);
        assert!(input_findings.iter().any(|f| f.rule == "GEN-CAS-DUPLICATE"));

        // The generator does not silently merge the two components.
        let result = draft_sections_from_resolved_input(&input, &HashMap::new());
        let rows = result
            .sds
            .composition
            .as_ref()
            .unwrap()
            .composition_and_concentration
            .as_ref()
            .unwrap();
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn invalid_concentration_range_is_mapped_as_supplied_not_repaired() {
        let input = ProductInput {
            trade_name: "Test".into(),
            other_names: vec![],
            supplier: supplier(),
            components: vec![range_component("7732-18-5", "Water", 90.0, 10.0)],
            measured_properties: Default::default(),
            evidence: vec![],
        };
        // Commit #8 flags this as invalid...
        let input_findings = validate_product_input(&input);
        assert!(input_findings
            .iter()
            .any(|f| f.rule == "GEN-CONC-RANGE-INVALID"));

        // ...but the generator maps it exactly as supplied, without
        // swapping bounds or otherwise "fixing" it.
        let result = draft_sections_from_resolved_input(&input, &HashMap::new());
        let conc = result
            .sds
            .composition
            .as_ref()
            .unwrap()
            .composition_and_concentration
            .as_ref()
            .unwrap()[0]
            .concentration
            .as_ref()
            .unwrap()
            .numeric_range_with_unit_and_qualifier
            .as_ref()
            .unwrap();
        assert_eq!(conc.lower_value.as_ref().unwrap().value, Some(90.0));
        assert_eq!(conc.upper_value.as_ref().unwrap().value, Some(10.0));
    }

    #[test]
    fn concentration_below_100_percent_gets_no_automatic_balance_component() {
        let input = ProductInput {
            trade_name: "Test".into(),
            other_names: vec![],
            supplier: supplier(),
            components: vec![exact_component("7732-18-5", "Water", 60.0)],
            measured_properties: Default::default(),
            evidence: vec![],
        };
        let result = draft_sections_from_resolved_input(&input, &HashMap::new());
        let rows = result
            .sds
            .composition
            .as_ref()
            .unwrap()
            .composition_and_concentration
            .as_ref()
            .unwrap();
        // Exactly one row — no synthetic "balance" component was added to
        // reach 100%.
        assert_eq!(rows.len(), 1);
    }

    #[test]
    fn concentration_above_threshold_finding_is_preserved() {
        let input = ProductInput {
            trade_name: "Test".into(),
            other_names: vec![],
            supplier: supplier(),
            components: vec![
                exact_component("7732-18-5", "Water", 60.0),
                exact_component("64-17-5", "Ethanol", 50.0),
            ],
            measured_properties: Default::default(),
            evidence: vec![],
        };
        let input_findings = validate_product_input(&input);
        assert!(input_findings
            .iter()
            .any(|f| f.rule == "GEN-CONC-SUM-EXCEEDS-100"));
    }

    #[test]
    fn serialized_draft_has_no_empty_artifacts_and_no_diagnostic_keys() {
        let input = ProductInput {
            trade_name: "Test Solvent".into(),
            other_names: vec![],
            supplier: SupplierInput {
                company_name: "Example Chemical Co.".into(),
                address: None,
                phone: None,
                email: None,
            },
            components: vec![exact_component("7732-18-5", "Water", 100.0)],
            measured_properties: Default::default(),
            evidence: vec![],
        };
        let result = draft_sections_from_resolved_input(&input, &HashMap::new());

        let value = serde_json::to_value(&result.sds).unwrap();
        let pruned = crate::converter::prune_empty_fields(value.clone());
        assert_eq!(
            value, pruned,
            "draft already matches pruned output — no empty placeholders were emitted"
        );

        // No generation-only diagnostics (findings, provenance, etc.) leak
        // into the official SDS JSON — `findings` lives on
        // `SectionDraftResult`, never inside `sds`.
        let as_object = value.as_object().unwrap();
        for key in ["findings", "provenance", "unresolved", "release_status"] {
            assert!(
                !as_object.contains_key(key),
                "official SDS JSON must not contain diagnostic key '{key}'"
            );
        }
    }
}
