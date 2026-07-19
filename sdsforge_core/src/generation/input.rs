use serde::{Deserialize, Serialize};

use super::evidence::{EvidenceSource, MeasuredPropertiesInput};

/// A single component's concentration in a mixture.
///
/// Either `exact` or a `lower`/`upper` pair should be set, not both — see
/// [`super::validate_product_input`] for the ambiguity check. `unit` is
/// required — there is no unambiguous default concentration unit.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConcentrationRange {
    pub exact: Option<f64>,
    pub lower: Option<f64>,
    pub upper: Option<f64>,
    pub unit: String,
}

/// One ingredient of a product formulation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComponentInput {
    pub cas_number: Option<String>,
    pub name: Option<String>,
    pub concentration: ConcentrationRange,
}

/// Supplier contact details for Section 1 (Identification).
///
/// Derives `Default` (empty/`None` fields) purely so `ProductInput`'s new
/// evidence-related fields (added alongside `MeasuredPropertiesInput`) can
/// be defaulted in existing test fixtures via `..Default::default()`
/// without hand-editing every construction site — not because a
/// default-valued supplier is ever meaningful on its own. `company_name`
/// stays required on deserialization regardless: an *omitted* `supplier`
/// key is a parse error (`ProductInput::supplier` has no `#[serde(default)]`),
/// and a *present* `supplier` object still requires `company_name`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SupplierInput {
    pub company_name: String,
    pub address: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
}

/// Raw input to the `generate` feature: a product's identity, supplier,
/// formulation, and any measured-property evidence. This is the domain
/// model — deliberately separate from the MHLW-schema `SdsRoot` in
/// [`crate::schema`], which is the *output* shape.
///
/// Derives `Default` for the same reason as [`SupplierInput`] — test
/// ergonomics for the new fields, not a meaningful default product.
/// Deliberately NOT `#[serde(default)]` on the struct itself: `trade_name`,
/// `supplier`, and `components` must remain required keys in the input
/// file. Only the three fields below whose omission has an unambiguous
/// meaning ("nothing was supplied") get `#[serde(default)]`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductInput {
    pub trade_name: String,
    #[serde(default)]
    pub other_names: Vec<String>,
    pub supplier: SupplierInput,
    pub components: Vec<ComponentInput>,
    /// Measured-property evidence for the seven safety-sensitive
    /// properties Section 1/3 generation alone can never resolve — see
    /// `docs/sdsforge-architecture.md`'s "Properties that require
    /// product-level evidence" table and
    /// `super::unresolved::PRODUCT_LEVEL_POLICIES`. Omitting this key
    /// means no measured-property evidence was supplied at all — the same
    /// as `measured_properties: {}` — never a parse error.
    #[serde(default)]
    pub measured_properties: MeasuredPropertiesInput,
    /// The evidence sources referenced by `measured_properties` entries'
    /// `evidence_id` fields. An entry whose `evidence_id` doesn't resolve
    /// here can never become `Confirmed` — see the resolution logic in
    /// `super::result`. Omitting this key means no evidence was supplied.
    #[serde(default)]
    pub evidence: Vec<EvidenceSource>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL_JSON: &str = r#"{
        "trade_name": "Minimal Product",
        "supplier": {"company_name": "Acme"},
        "components": [
            {"concentration": {"exact": 100.0, "unit": "%"}}
        ]
    }"#;

    const VERBOSE_EQUIVALENT_JSON: &str = r#"{
        "trade_name": "Minimal Product",
        "other_names": [],
        "supplier": {"company_name": "Acme", "address": null, "phone": null, "email": null},
        "components": [
            {"cas_number": null, "name": null,
             "concentration": {"exact": 100.0, "lower": null, "upper": null, "unit": "%"}}
        ],
        "measured_properties": {"flash_point": [], "boiling_point": [], "vapor_pressure": [],
            "explosive_limits": [], "self_reactivity": [], "oxidizing_properties": [],
            "metal_corrosivity": []},
        "evidence": []
    }"#;

    #[test]
    fn minimal_json_omits_other_names_measured_properties_and_evidence() {
        let input: ProductInput = serde_json::from_str(MINIMAL_JSON).unwrap();
        assert_eq!(input.trade_name, "Minimal Product");
        assert!(input.other_names.is_empty());
        assert!(input.evidence.is_empty());
        assert!(input.measured_properties.flash_point.is_empty());
        assert!(input.measured_properties.boiling_point.is_empty());
        assert!(input.components[0].cas_number.is_none());
        assert!(input.components[0].name.is_none());
    }

    #[test]
    fn verbose_and_concise_product_input_deserialize_to_equivalent_values() {
        let concise: ProductInput = serde_json::from_str(MINIMAL_JSON).unwrap();
        let verbose: ProductInput = serde_json::from_str(VERBOSE_EQUIVALENT_JSON).unwrap();
        assert_eq!(
            serde_json::to_string(&concise).unwrap(),
            serde_json::to_string(&verbose).unwrap()
        );
    }

    #[test]
    fn missing_trade_name_fails() {
        let json = r#"{"supplier":{"company_name":"Acme"},"components":[]}"#;
        assert!(serde_json::from_str::<ProductInput>(json).is_err());
    }

    #[test]
    fn missing_supplier_fails() {
        let json = r#"{"trade_name":"X","components":[]}"#;
        assert!(serde_json::from_str::<ProductInput>(json).is_err());
    }

    #[test]
    fn missing_supplier_company_name_fails() {
        let json = r#"{"trade_name":"X","supplier":{},"components":[]}"#;
        assert!(serde_json::from_str::<ProductInput>(json).is_err());
    }

    #[test]
    fn missing_components_fails() {
        let json = r#"{"trade_name":"X","supplier":{"company_name":"Acme"}}"#;
        assert!(serde_json::from_str::<ProductInput>(json).is_err());
    }

    #[test]
    fn missing_component_concentration_fails() {
        let json = r#"{"cas_number":"7732-18-5","name":"Water"}"#;
        assert!(serde_json::from_str::<ComponentInput>(json).is_err());
    }

    #[test]
    fn missing_concentration_unit_fails() {
        let json = r#"{"exact": 100.0}"#;
        assert!(serde_json::from_str::<ConcentrationRange>(json).is_err());
    }

    #[test]
    fn unknown_top_level_field_fails() {
        let json = r#"{"trade_name":"X","supplier":{"company_name":"Acme"},
            "components":[],"typo_field": 1}"#;
        let err = serde_json::from_str::<ProductInput>(json).unwrap_err();
        assert!(err.to_string().contains("typo_field"));
    }

    #[test]
    fn unknown_component_field_fails() {
        let json = r#"{"concentration":{"exact":1.0,"unit":"%"},"bogus":true}"#;
        let err = serde_json::from_str::<ComponentInput>(json).unwrap_err();
        assert!(err.to_string().contains("bogus"));
    }

    #[test]
    fn misspelled_concentration_field_fails_instead_of_being_ignored() {
        // "concentation" (missing r) is silently dropped by deny_unknown_fields
        // as an unknown key, which then surfaces the real problem: the
        // required `concentration` field is missing.
        let json =
            r#"{"cas_number":"7732-18-5","name":"Water","concentation":{"exact":1.0,"unit":"%"}}"#;
        assert!(serde_json::from_str::<ComponentInput>(json).is_err());
    }

    #[test]
    fn explicit_empty_trade_name_is_not_replaced_by_default() {
        let json = r#"{"trade_name":"","supplier":{"company_name":"Acme"},"components":[]}"#;
        let input: ProductInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.trade_name, "");
    }

    #[test]
    fn explicit_empty_components_parses_distinctly_from_a_missing_components_key() {
        // `components` has no `#[serde(default)]` -- an explicit `[]` is a
        // different code path from an omitted key (see
        // `missing_components_fails`): the parser accepts an explicit empty
        // list, whether or not a downstream formulation check later flags
        // it, rather than treating "empty" and "absent" as interchangeable.
        let json = r#"{"trade_name":"X","supplier":{"company_name":"Acme"},"components":[]}"#;
        let input: ProductInput = serde_json::from_str(json).unwrap();
        assert!(input.components.is_empty());
    }
}
