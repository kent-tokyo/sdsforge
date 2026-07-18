use serde::{Deserialize, Serialize};

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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupplierInput {
    pub company_name: String,
    pub address: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
}

/// Raw input to the `generate` feature: a product's identity, supplier, and
/// formulation. This is the domain model — deliberately separate from the
/// MHLW-schema `SdsRoot` in [`crate::schema`], which is the *output* shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductInput {
    pub trade_name: String,
    pub other_names: Vec<String>,
    pub supplier: SupplierInput,
    pub components: Vec<ComponentInput>,
}
