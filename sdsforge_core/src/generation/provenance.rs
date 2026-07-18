use serde::{Deserialize, Serialize};

/// How confident the generator is in a value, independent of where the
/// value came from. A `SupplierSpecification` source and a
/// `ProductTestReport` source can both be `High` confidence; an
/// `UnverifiedUserInput` source is `Unverified` by construction — nothing
/// about passing structural validation (CAS check-digit, concentration
/// bounds) upgrades it, since syntactic validity isn't evidence that a
/// company name, address, or product identity is factually correct.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfidenceLevel {
    High,
    Medium,
    Low,
    Unverified,
}

/// Priority-ordered evidence source. Not every level is valid for every
/// field — see [`super::unresolved::FieldPolicy::allowed_evidence`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceLevel {
    ProductTestReport,
    EquivalentBatchTestReport,
    SupplierSpecification,
    SupplierSds,
    RegulatoryDatabase,
    PeerReviewedLiterature,
    ReferenceDatabase,
    DeterministicCalculation,
    ModelEstimate,
    UnverifiedUserInput,
    None,
}

/// Measurement conditions for a physical/chemical property. Kept small —
/// this only needs enough structure to say what evidence the user must
/// provide, not to model a general laboratory information system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeasurementConditions {
    pub temperature_c: Option<f64>,
    pub pressure_kpa: Option<f64>,
    pub atmosphere: Option<String>,
}

/// A single field's origin: where its value came from, how, and how
/// confident the generator is in it. One `FieldProvenance` per populated
/// field.
///
/// `path` uses dot-separated MHLW JSON key names (the same names
/// `#[serde(rename = "...")]` puts on the wire in `official_sds.json`), with
/// `[i]` for array indices — e.g.
/// `"Composition.CompositionAndConcentration[0].SubstanceIdentifiers.SubstanceNames.GenericName"`.
/// This is the one canonical path convention for this feature; see
/// `path_helpers` below and `tests::path_convention_is_stable`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldProvenance {
    pub path: String,
    pub source_type: EvidenceLevel,
    pub source_reference: Option<String>,
    pub source_value: Option<String>,
    pub method: String,
    pub sample_id: Option<String>,
    pub batch_id: Option<String>,
    pub test_method: Option<String>,
    pub conditions: Option<MeasurementConditions>,
    /// Deliberately left `None` by every code path in this commit — no
    /// live timestamp is generated, so repeated generation from the same
    /// input produces byte-identical report JSON.
    pub retrieved_at: Option<String>,
    pub confidence: ConfidenceLevel,
    pub warnings: Vec<String>,
}

impl FieldProvenance {
    /// A value the caller typed into `ProductInput` directly — trade name,
    /// supplier contact fields, component name/CAS/concentration. Always
    /// `UnverifiedUserInput` / `Unverified` confidence: structural
    /// validation (commit #8) confirms the *shape* is well-formed, not that
    /// the content is factually correct.
    pub fn supplied(path: impl Into<String>, method: impl Into<String>) -> Self {
        FieldProvenance {
            path: path.into(),
            source_type: EvidenceLevel::UnverifiedUserInput,
            source_reference: None,
            source_value: None,
            method: method.into(),
            sample_id: None,
            batch_id: None,
            test_method: None,
            conditions: None,
            retrieved_at: None,
            confidence: ConfidenceLevel::Unverified,
            warnings: Vec::new(),
        }
    }

    /// A value returned by the existing CAS enrichment resolver
    /// ([`crate::enrichment::lookup_cas`]). `ReferenceDatabase`, not
    /// `Confirmed`/`DeterministicCalculation` — a CAS lookup result is
    /// evidence about a chemical identity *candidate*, not proof that the
    /// supplied product or batch actually contains that substance at the
    /// declared concentration, and this commit records the resolver's
    /// returned value rather than recalculating it from structure.
    pub fn from_cas_resolver(path: impl Into<String>, pubchem_cid: Option<u64>) -> Self {
        FieldProvenance {
            path: path.into(),
            source_type: EvidenceLevel::ReferenceDatabase,
            source_reference: pubchem_cid.map(|cid| format!("PubChem CID {cid}")),
            source_value: None,
            method: "CAS lookup through existing enrichment layer (PubChem)".into(),
            sample_id: None,
            batch_id: None,
            test_method: None,
            conditions: None,
            retrieved_at: None,
            confidence: ConfidenceLevel::Medium,
            warnings: Vec::new(),
        }
    }
}

/// Canonical MHLW JSON path fragments, so every call site builds paths the
/// same way instead of hand-formatting strings.
pub mod path {
    pub const TRADE_NAME_JP: &str = "Identification.TradeProductIdentity.TradeNameJP";
    pub const OTHER_NAME: &str = "Identification.TradeProductIdentity.OtherName";
    pub const SUPPLIER_COMPANY_NAME: &str = "Identification.SupplierInformation.CompanyName";
    pub const SUPPLIER_ADDRESS: &str = "Identification.SupplierInformation.Address";
    pub const SUPPLIER_PHONE: &str = "Identification.SupplierInformation.Phone";
    pub const SUPPLIER_EMAIL: &str = "Identification.SupplierInformation.Email";

    pub fn composition_row(index: usize, field: &str) -> String {
        format!("Composition.CompositionAndConcentration[{index}].{field}")
    }

    pub const GENERIC_NAME: &str = "SubstanceIdentifiers.SubstanceNames.GenericName";
    pub const IUPAC_NAME: &str = "SubstanceIdentifiers.SubstanceNames.IupacName";
    pub const CAS_NO: &str = "SubstanceIdentifiers.SubstanceIdentity.CASno";
    pub const CONCENTRATION: &str = "Concentration";
    pub const MOLECULAR_FORMULA: &str = "MolecularFormula";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_convention_is_stable() {
        assert_eq!(
            path::TRADE_NAME_JP,
            "Identification.TradeProductIdentity.TradeNameJP"
        );
        assert_eq!(
            path::composition_row(0, path::GENERIC_NAME),
            "Composition.CompositionAndConcentration[0].SubstanceIdentifiers.SubstanceNames.GenericName"
        );
        assert_eq!(
            path::composition_row(2, path::CAS_NO),
            "Composition.CompositionAndConcentration[2].SubstanceIdentifiers.SubstanceIdentity.CASno"
        );
    }

    #[test]
    fn supplied_provenance_is_unverified() {
        let p = FieldProvenance::supplied(path::TRADE_NAME_JP, "supplied in ProductInput");
        assert_eq!(p.source_type, EvidenceLevel::UnverifiedUserInput);
        assert_eq!(p.confidence, ConfidenceLevel::Unverified);
    }

    #[test]
    fn cas_resolver_provenance_is_reference_database_not_confirmed() {
        let p = FieldProvenance::from_cas_resolver(
            path::composition_row(0, path::IUPAC_NAME),
            Some(962),
        );
        assert_eq!(p.source_type, EvidenceLevel::ReferenceDatabase);
        assert_eq!(p.source_reference.as_deref(), Some("PubChem CID 962"));
    }

    #[test]
    fn evidence_level_serializes_snake_case() {
        let json = serde_json::to_string(&EvidenceLevel::UnverifiedUserInput).unwrap();
        assert_eq!(json, "\"unverified_user_input\"");
        let json = serde_json::to_string(&EvidenceLevel::ReferenceDatabase).unwrap();
        assert_eq!(json, "\"reference_database\"");
    }
}
