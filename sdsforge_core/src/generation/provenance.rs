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

    /// A property resolved from caller-supplied measured-value evidence
    /// (flash point, boiling point, vapor pressure, explosive limits) that
    /// fully satisfied its `FieldPolicy` — see
    /// `super::resolve::resolve_measured_properties`. `confidence: High`
    /// and `source_type` taken directly from the [`super::EvidenceSource`]
    /// that resolved it (`ProductTestReport`/`EquivalentBatchTestReport`/
    /// `SupplierSpecification`, whichever this property's policy accepted)
    /// — never upgraded to a status the evidence itself doesn't support.
    pub fn from_measured_evidence(
        path: impl Into<String>,
        source: &super::EvidenceSource,
        method: Option<&str>,
        sample_id: Option<&str>,
        batch_id: Option<&str>,
        conditions: &MeasurementConditions,
    ) -> Self {
        FieldProvenance {
            path: path.into(),
            source_type: source.level,
            source_reference: Some(source.reference.clone()),
            source_value: None,
            method: "resolved from supplied measured-property evidence".into(),
            sample_id: sample_id.map(str::to_string),
            batch_id: batch_id.map(str::to_string),
            test_method: method.map(str::to_string),
            conditions: Some(conditions.clone()),
            retrieved_at: None,
            confidence: ConfidenceLevel::High,
            warnings: Vec::new(),
        }
    }

    /// Same as [`Self::from_measured_evidence`] for the three free-text-only
    /// properties (self-reactivity, oxidizing properties, metal
    /// corrosivity), whose evidence is a [`super::TestResultEvidence`]
    /// rather than a numeric value.
    pub fn from_test_result_evidence(
        path: impl Into<String>,
        source: &super::EvidenceSource,
        result: &super::TestResultEvidence,
    ) -> Self {
        FieldProvenance {
            path: path.into(),
            source_type: source.level,
            source_reference: Some(source.reference.clone()),
            source_value: Some(result.result.clone()),
            method: "resolved from supplied measured-property evidence".into(),
            sample_id: result.sample_id.clone(),
            batch_id: result.batch_id.clone(),
            test_method: result.method.clone(),
            conditions: None,
            retrieved_at: None,
            confidence: ConfidenceLevel::High,
            warnings: Vec::new(),
        }
    }

    /// The candidate's own structure as PubChem returned it, before any
    /// normalization — `ReferenceDatabase`, same evidence level as
    /// [`Self::from_cas_resolver`], never `Confirmed`. Applies equally to
    /// either PubChem representation actually used as the normalization
    /// input: the full `SMILES` property (stereochemistry/isotopes where
    /// represented) or, only as a fallback, `ConnectivitySMILES`
    /// (connectivity only) — neither is chematic's separately-provenanced,
    /// `DeterministicCalculation`-level canonical form (see
    /// [`Self::canonical_smiles`]). `used_connectivity_fallback` records
    /// which one so the report can disclose when stereochemistry/isotope
    /// information may not be represented.
    pub fn source_smiles(pubchem_cid: Option<u64>, used_connectivity_fallback: bool) -> Self {
        FieldProvenance {
            path: path::SOURCE_SMILES.to_string(),
            source_type: EvidenceLevel::ReferenceDatabase,
            source_reference: pubchem_cid.map(|cid| format!("PubChem CID {cid}")),
            source_value: None,
            method: if used_connectivity_fallback {
                "PubChem CAS lookup (ConnectivitySMILES fallback)".into()
            } else {
                "PubChem CAS lookup (SMILES)".into()
            },
            sample_id: None,
            batch_id: None,
            test_method: None,
            conditions: None,
            retrieved_at: None,
            confidence: ConfidenceLevel::Medium,
            warnings: if used_connectivity_fallback {
                vec![
                    "connectivity-only SMILES used as fallback -- stereochemistry/isotope \
                     information may not be represented"
                        .to_string(),
                ]
            } else {
                Vec::new()
            },
        }
    }

    /// The chematic-canonicalized SMILES actually written into
    /// `official_sds.json`'s `SMILES` field. `DeterministicCalculation` —
    /// a canonical form is a deterministic transformation for this
    /// chematic version and input, not proof the underlying chemical
    /// identity is confirmed. Never `Confirmed`/`ProductTestReport`/
    /// `EquivalentBatchTestReport` — the evidence level is hardcoded here,
    /// not something a caller can override.
    pub fn canonical_smiles(pubchem_cid: Option<u64>, warnings: Vec<String>) -> Self {
        FieldProvenance {
            path: path::CANONICAL_SMILES.to_string(),
            source_type: EvidenceLevel::DeterministicCalculation,
            source_reference: pubchem_cid.map(|cid| format!("PubChem CID {cid}")),
            source_value: None,
            method: "chematic 0.4.30 canonical_smiles".into(),
            sample_id: None,
            batch_id: None,
            test_method: None,
            conditions: None,
            retrieved_at: None,
            confidence: ConfidenceLevel::Medium,
            warnings,
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
    /// The one real schema field (`CompositionCompositionAndConcentration.smiles`,
    /// `#[serde(rename = "SMILES")]`) — only the chematic-canonicalized value
    /// is ever written here.
    pub const CANONICAL_SMILES: &str = "SMILES";
    /// Report-only path — the schema has no separate "pre-normalization
    /// SMILES" field, so the resolver's own value (before chematic
    /// canonicalization) only ever appears in `generation_report`
    /// provenance, never in `official_sds.json`.
    pub const SOURCE_SMILES: &str = "Structure.SourceSmiles (report-only, no schema field)";
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
    fn source_smiles_full_form_has_no_fallback_warning() {
        let p = FieldProvenance::source_smiles(Some(702), false);
        assert_eq!(p.source_type, EvidenceLevel::ReferenceDatabase);
        assert_eq!(p.method, "PubChem CAS lookup (SMILES)");
        assert!(p.warnings.is_empty());
    }

    #[test]
    fn source_smiles_connectivity_fallback_discloses_stereo_loss() {
        let p = FieldProvenance::source_smiles(Some(702), true);
        assert_eq!(p.source_type, EvidenceLevel::ReferenceDatabase);
        assert_eq!(p.method, "PubChem CAS lookup (ConnectivitySMILES fallback)");
        assert_eq!(p.warnings.len(), 1);
        assert!(p.warnings[0].contains("stereochemistry/isotope"));
    }

    #[test]
    fn evidence_level_serializes_snake_case() {
        let json = serde_json::to_string(&EvidenceLevel::UnverifiedUserInput).unwrap();
        assert_eq!(json, "\"unverified_user_input\"");
        let json = serde_json::to_string(&EvidenceLevel::ReferenceDatabase).unwrap();
        assert_eq!(json, "\"reference_database\"");
    }
}
