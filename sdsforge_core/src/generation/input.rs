use serde::{Deserialize, Serialize};

use super::evidence::{EvidenceSource, MeasuredPropertiesInput};

/// A single component's concentration in a mixture.
///
/// Either `exact` or a `lower`/`upper` pair should be set, not both — see
/// [`super::validate_product_input`] for the ambiguity check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConcentrationRange {
    pub exact: Option<f64>,
    pub lower: Option<f64>,
    pub upper: Option<f64>,
    pub unit: String,
}

/// One ingredient of a product formulation.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
/// default-valued supplier is ever meaningful on its own.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProductInput {
    pub trade_name: String,
    pub other_names: Vec<String>,
    pub supplier: SupplierInput,
    pub components: Vec<ComponentInput>,
    /// Measured-property evidence for the seven safety-sensitive
    /// properties Section 1/3 generation alone can never resolve — see
    /// `docs/sdsforge-architecture.md`'s "Properties that require
    /// product-level evidence" table and
    /// `super::unresolved::PRODUCT_LEVEL_POLICIES`.
    pub measured_properties: MeasuredPropertiesInput,
    /// The evidence sources referenced by `measured_properties` entries'
    /// `evidence_id` fields. An entry whose `evidence_id` doesn't resolve
    /// here can never become `Confirmed` — see the resolution logic in
    /// `super::result`.
    pub evidence: Vec<EvidenceSource>,
}
