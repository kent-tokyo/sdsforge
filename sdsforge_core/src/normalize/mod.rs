//! Chemical structure normalization — a peer to [`crate::enrichment`], not
//! nested inside it or inside [`crate::generation`]. Mirrors the boundary
//! `docs/sdsforge-architecture.md`'s "chematic integration boundary" section
//! describes:
//!
//! ```text
//! enrichment  — resolves CAS -> candidate record(s) (PubChem)
//! normalize   — parses/canonicalizes/flags-inconsistency on a candidate
//!               enrichment already resolved (this module)
//! generation  — decides whether the result is usable, records provenance
//!               and uncertainty, maps verified data into the SDS draft
//! ```
//!
//! A normalizer never resolves CAS numbers, never predicts finished-product
//! properties (flash point, vapor pressure, GHS classification, ...), and
//! never becomes evidence for anything [`crate::generation`]'s
//! `PRODUCT_LEVEL_POLICIES` already governs — those stay exactly as commits
//! 2ac2758/d4dd15d ("A") left them, untouched by this module.

#[cfg(feature = "chematic-normalization")]
mod chematic_impl;
#[cfg(feature = "chematic-normalization")]
pub use chematic_impl::ChematicNormalizer;

use crate::enrichment::ChemicalIdentityCandidate;

/// A local, deterministic parse/canonicalize/consistency-check step over one
/// already-resolved [`ChemicalIdentityCandidate`]. Implementations must not
/// perform network I/O (normalization is local by design) and must not
/// silently replace the candidate's identity (e.g. by stripping a salt) —
/// see [`ChemicalNormalizationResult`]'s doc comment.
pub trait ChemicalNormalizer {
    fn normalize(&self, candidate: &ChemicalIdentityCandidate) -> ChemicalNormalizationResult;
}

/// Explicit behavior when chematic support isn't compiled in — distinct from
/// "no SMILES was supplied" ([`NormalizationStatus::MissingStructure`]) so a
/// caller can tell the two situations apart.
pub struct UnavailableNormalizer;

impl ChemicalNormalizer for UnavailableNormalizer {
    fn normalize(&self, candidate: &ChemicalIdentityCandidate) -> ChemicalNormalizationResult {
        match &candidate.source_smiles {
            None => ChemicalNormalizationResult {
                original_smiles: None,
                canonical_smiles: None,
                status: NormalizationStatus::MissingStructure,
                issues: vec![],
                calculated: CalculatedIdentityProperties::default(),
                screening_alerts: vec![],
            },
            Some(smiles) => ChemicalNormalizationResult {
                original_smiles: Some(smiles.clone()),
                canonical_smiles: None,
                status: NormalizationStatus::ReviewRequired,
                issues: vec![],
                calculated: CalculatedIdentityProperties::default(),
                screening_alerts: vec![],
            },
        }
    }
}

/// Result of normalizing one candidate. `canonical_smiles` is a
/// **deterministic transformation of the input for this chematic version**,
/// not a confirmation of chemical identity — a candidate a normalizer
/// cannot process at all (missing/invalid structure, or the
/// `chematic-normalization` feature disabled) still returns supplied
/// CAS/name/concentration data untouched elsewhere in the generation
/// pipeline; this result only ever adds information, it never causes
/// already-resolved fields to be discarded.
#[derive(Debug, Clone)]
pub struct ChemicalNormalizationResult {
    /// The candidate's own SMILES, unchanged, kept separate from anything
    /// derived from it.
    pub original_smiles: Option<String>,
    pub canonical_smiles: Option<String>,
    pub status: NormalizationStatus,
    pub issues: Vec<NormalizationIssue>,
    pub calculated: CalculatedIdentityProperties,
    /// PAINS/Brenk substructure alert names — screening-only, never a GHS
    /// classification or confirmed hazard, never product-level evidence,
    /// never blocks release by itself.
    pub screening_alerts: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NormalizationStatus {
    Normalized,
    MissingStructure,
    InvalidStructure,
    Ambiguous,
    ReviewRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NormalizationIssue {
    InvalidSmiles,
    MultipleResolverCandidates,
    /// A dot-disconnected structure (e.g. a salt or solvate) — reported,
    /// never silently reduced to its largest fragment. See
    /// `docs/sdsforge-architecture.md`: a salt/solvate/charged form may be
    /// the *correct* identity for the CAS number; picking the largest
    /// fragment could silently turn it into a different substance.
    MultiFragmentStructure,
    FormulaMismatch,
    ChargeOrSaltPresent,
    UnsupportedPolymerOrMixture,
    StereochemistryNotFullyPreserved,
}

/// Deterministic calculations over the parsed structure — never an
/// invented value. Every field is `None` when the underlying chematic
/// operation wasn't run (e.g. `UnavailableNormalizer`, or parsing failed).
#[derive(Debug, Clone, Default)]
pub struct CalculatedIdentityProperties {
    pub molecular_formula: Option<String>,
    pub molecular_weight: Option<f64>,
    pub formal_charge_sum: Option<i32>,
    /// Whether the structure has more than one disconnected fragment.
    /// Deliberately not a `fragment_count: usize` — chematic's
    /// `largest_fragment` only gives a yes/no signal (does the largest
    /// fragment have fewer atoms than the whole structure?), not an exact
    /// count; claiming a precise count would invent a value the underlying
    /// operation doesn't actually provide.
    pub has_multiple_fragments: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(smiles: Option<&str>) -> ChemicalIdentityCandidate {
        ChemicalIdentityCandidate {
            cas: "7732-18-5".into(),
            pubchem_cid: Some(962),
            iupac_name: Some("oxidane".into()),
            molecular_formula: Some("H2O".into()),
            source_smiles: smiles.map(str::to_string),
            isomeric_smiles: None,
            inchi_key: None,
        }
    }

    #[test]
    fn unavailable_normalizer_reports_missing_structure_when_no_smiles() {
        let result = UnavailableNormalizer.normalize(&candidate(None));
        assert_eq!(result.status, NormalizationStatus::MissingStructure);
    }

    #[test]
    fn unavailable_normalizer_reports_review_required_when_smiles_present() {
        // Distinct from "no SMILES supplied" — chematic just isn't compiled in.
        let result = UnavailableNormalizer.normalize(&candidate(Some("O")));
        assert_eq!(result.status, NormalizationStatus::ReviewRequired);
        assert!(result.canonical_smiles.is_none());
    }

    /// Feature-disabled sanity check: this whole crate, including this
    /// test, must compile and pass without `chematic-normalization`.
    #[cfg(not(feature = "chematic-normalization"))]
    #[test]
    fn crate_compiles_without_chematic_normalization_feature() {
        // Reaching this assertion at all is the test.
    }
}
