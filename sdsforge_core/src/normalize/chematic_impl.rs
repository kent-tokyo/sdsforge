//! [`ChematicNormalizer`] — the real implementation, backed by the
//! `chematic` crate (feature-gated: `chematic-normalization`).
//!
//! Uses only public `chematic` APIs, verified against the actual published
//! source (`kent-tokyo/chematic` at 0.4.30) rather than assumed:
//! `chematic::smiles::parse`/`canonical_smiles`,
//! `chematic::chem::{calc_mol_formula, formal_charge_sum, largest_fragment,
//! molecular_weight, parse_formula, pains_matches, brenk_matches}`.
//!
//! Deliberately never calls `chematic::chem::standardize`/`uncharge`/
//! `neutralize_charges`/`canonical_tautomer` and overwrites the candidate
//! with the result — a salt, solvate, charged form, or multi-fragment
//! structure may be the identity actually associated with the CAS number.
//! `largest_fragment` is called only to *detect* a multi-fragment structure
//! (comparing atom counts), never to replace what's reported.

use chematic::chem;
use chematic::smiles;

use crate::enrichment::ChemicalIdentityCandidate;

use super::{
    CalculatedIdentityProperties, ChemicalNormalizationResult, ChemicalNormalizer,
    NormalizationIssue, NormalizationStatus,
};

pub struct ChematicNormalizer;

impl ChemicalNormalizer for ChematicNormalizer {
    fn normalize(&self, candidate: &ChemicalIdentityCandidate) -> ChemicalNormalizationResult {
        let Some(source_smiles) = &candidate.source_smiles else {
            return ChemicalNormalizationResult {
                original_smiles: None,
                canonical_smiles: None,
                status: NormalizationStatus::MissingStructure,
                issues: vec![],
                calculated: CalculatedIdentityProperties::default(),
                screening_alerts: vec![],
            };
        };

        let mol = match smiles::parse(source_smiles) {
            Ok(mol) => mol,
            Err(_) => {
                // The candidate's IUPAC name / CID / formula from PubChem
                // may still be independently usable — an invalid SMILES
                // string is a data-quality issue with this one field, not
                // proof the whole candidate is wrong.
                return ChemicalNormalizationResult {
                    original_smiles: Some(source_smiles.clone()),
                    canonical_smiles: None,
                    status: NormalizationStatus::InvalidStructure,
                    issues: vec![NormalizationIssue::InvalidSmiles],
                    calculated: CalculatedIdentityProperties::default(),
                    screening_alerts: vec![],
                };
            }
        };

        let canonical = smiles::canonical_smiles(&mol);
        let mut issues = Vec::new();

        // Multi-fragment detection: compare atom counts before/after
        // largest_fragment. The largest-fragment molecule itself is
        // discarded immediately after this check -- never stored, never
        // returned as the identity.
        let has_multiple_fragments = chem::largest_fragment(&mol).atom_count() < mol.atom_count();
        if has_multiple_fragments {
            issues.push(NormalizationIssue::MultiFragmentStructure);
        }

        let formal_charge_sum = chem::formal_charge_sum(&mol);
        if formal_charge_sum != 0 {
            issues.push(NormalizationIssue::ChargeOrSaltPresent);
        }

        let calculated_formula = chem::calc_mol_formula(&mol);
        if let Some(resolver_formula) = &candidate.molecular_formula {
            if !formulas_equivalent(resolver_formula, &calculated_formula) {
                issues.push(NormalizationIssue::FormulaMismatch);
            }
        }

        let screening_alerts: Vec<String> = chem::pains_matches(&mol)
            .into_iter()
            .chain(chem::brenk_matches(&mol))
            .map(str::to_string)
            .collect();

        let status = if issues.iter().any(|i| {
            matches!(
                i,
                NormalizationIssue::FormulaMismatch | NormalizationIssue::MultiFragmentStructure
            )
        }) {
            NormalizationStatus::ReviewRequired
        } else {
            NormalizationStatus::Normalized
        };

        ChemicalNormalizationResult {
            original_smiles: Some(source_smiles.clone()),
            canonical_smiles: Some(canonical),
            status,
            issues,
            calculated: CalculatedIdentityProperties {
                molecular_formula: Some(calculated_formula),
                molecular_weight: Some(chem::molecular_weight(&mol)),
                formal_charge_sum: Some(formal_charge_sum),
                has_multiple_fragments: Some(has_multiple_fragments),
            },
            screening_alerts,
        }
    }
}

/// Structural comparison via `chematic::chem::parse_formula` (an existing,
/// reliable implementation — not a home-grown parser written for this
/// commit) rather than raw string equality, so harmless element-ordering
/// differences ("H2O" vs "OH2") don't register as a mismatch.
fn formulas_equivalent(a: &str, b: &str) -> bool {
    match (chem::parse_formula(a), chem::parse_formula(b)) {
        (Ok(pa), Ok(pb)) => pa == pb,
        _ => a == b,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(smiles: Option<&str>, formula: Option<&str>) -> ChemicalIdentityCandidate {
        ChemicalIdentityCandidate {
            cas: "test".into(),
            pubchem_cid: Some(1),
            iupac_name: None,
            molecular_formula: formula.map(str::to_string),
            source_smiles: smiles.map(str::to_string),
            isomeric_smiles: None,
            inchi_key: None,
        }
    }

    #[test]
    fn valid_smiles_parses_and_canonicalizes() {
        let result = ChematicNormalizer.normalize(&candidate(Some("CCO"), None));
        assert_eq!(result.status, NormalizationStatus::Normalized);
        assert!(result.canonical_smiles.is_some());
    }

    #[test]
    fn canonicalization_is_idempotent() {
        // canonical_smiles(parse(x)) round-trips: re-parsing the canonical
        // form and re-canonicalizing yields the same string.
        let mol = smiles::parse("c1ccccc1").unwrap(); // benzene
        let c1 = smiles::canonical_smiles(&mol);
        let c2 = smiles::canonical_smiles(&smiles::parse(&c1).unwrap());
        assert_eq!(c1, c2);
    }

    #[test]
    fn invalid_smiles_produces_invalid_structure_no_panic() {
        let result = ChematicNormalizer.normalize(&candidate(Some("not-a-smiles((("), None));
        assert_eq!(result.status, NormalizationStatus::InvalidStructure);
        assert!(result.issues.contains(&NormalizationIssue::InvalidSmiles));
        assert!(result.canonical_smiles.is_none());
    }

    #[test]
    fn missing_smiles_reports_missing_structure_without_fabrication() {
        let result = ChematicNormalizer.normalize(&candidate(None, None));
        assert_eq!(result.status, NormalizationStatus::MissingStructure);
        assert!(result.canonical_smiles.is_none());
        assert!(result.calculated.molecular_formula.is_none());
    }

    #[test]
    fn salt_is_not_reduced_to_largest_fragment() {
        // Sodium chloride: two disconnected ionic fragments.
        let result = ChematicNormalizer.normalize(&candidate(Some("[Na+].[Cl-]"), None));
        assert!(result
            .issues
            .contains(&NormalizationIssue::MultiFragmentStructure));
        assert_eq!(result.calculated.has_multiple_fragments, Some(true));
        // Canonical SMILES still represents BOTH fragments -- not reduced
        // to a single ion.
        assert!(result.canonical_smiles.as_deref().unwrap().contains('.'));
    }

    #[test]
    fn multi_fragment_structure_is_review_required_not_blocked_by_normalizer() {
        let result = ChematicNormalizer.normalize(&candidate(Some("[Na+].[Cl-]"), None));
        // The normalizer itself never "blocks" -- that's a generation-layer
        // decision from the finding severity. It reports ReviewRequired.
        assert_eq!(result.status, NormalizationStatus::ReviewRequired);
    }

    #[test]
    fn charged_structure_remains_charged() {
        let result = ChematicNormalizer.normalize(&candidate(Some("[NH4+]"), None));
        assert!(result
            .issues
            .contains(&NormalizationIssue::ChargeOrSaltPresent));
        assert_eq!(result.calculated.formal_charge_sum, Some(1));
        // Never neutralized in the returned canonical SMILES.
        assert!(result.canonical_smiles.as_deref().unwrap().contains('+'));
    }

    #[test]
    fn matching_formula_produces_no_mismatch_issue() {
        let result = ChematicNormalizer.normalize(&candidate(Some("CCO"), Some("C2H6O")));
        assert!(!result.issues.contains(&NormalizationIssue::FormulaMismatch));
        assert_eq!(result.status, NormalizationStatus::Normalized);
    }

    #[test]
    fn mismatched_formula_flagged_and_calculated_value_retained() {
        let result = ChematicNormalizer.normalize(&candidate(Some("CCO"), Some("C6H12O6")));
        assert!(result.issues.contains(&NormalizationIssue::FormulaMismatch));
        assert_eq!(result.status, NormalizationStatus::ReviewRequired);
        // The calculated value is retained even though it disagrees with
        // the resolver-supplied one -- both are available to the caller.
        assert_eq!(
            result.calculated.molecular_formula.as_deref(),
            Some("C2H6O")
        );
    }

    #[test]
    fn no_product_level_property_field_exists_on_the_result() {
        // Structural guarantee, not just a runtime check: CalculatedIdentityProperties
        // has exactly these four fields and none of flash_point/vapor_pressure/etc.
        let calc = CalculatedIdentityProperties::default();
        let CalculatedIdentityProperties {
            molecular_formula,
            molecular_weight,
            formal_charge_sum,
            has_multiple_fragments,
        } = calc;
        let _ = (
            molecular_formula,
            molecular_weight,
            formal_charge_sum,
            has_multiple_fragments,
        );
    }

    #[test]
    fn repeated_normalization_is_byte_equivalent() {
        let c = candidate(Some("c1ccccc1"), Some("C6H6"));
        let a = ChematicNormalizer.normalize(&c);
        let b = ChematicNormalizer.normalize(&c);
        assert_eq!(a.canonical_smiles, b.canonical_smiles);
        assert_eq!(
            a.calculated.molecular_formula,
            b.calculated.molecular_formula
        );
    }

    #[test]
    fn stereochemical_smiles_parses_without_panic() {
        let result = ChematicNormalizer.normalize(&candidate(Some("F/C=C/F"), None));
        assert_ne!(result.status, NormalizationStatus::InvalidStructure);
    }

    #[test]
    fn isotopically_labelled_smiles_parses_without_panic() {
        let result = ChematicNormalizer.normalize(&candidate(Some("[13CH4]"), None));
        assert_ne!(result.status, NormalizationStatus::InvalidStructure);
    }

    #[test]
    fn very_long_smiles_does_not_panic() {
        let long_chain = "C".repeat(500);
        let result = ChematicNormalizer.normalize(&candidate(Some(&long_chain), None));
        // Whatever the outcome, it must not panic -- reaching this line is the test.
        let _ = result.status;
    }

    #[test]
    fn aromatic_molecule_normalizes() {
        let result = ChematicNormalizer.normalize(&candidate(Some("c1ccccc1"), Some("C6H6")));
        assert_eq!(result.status, NormalizationStatus::Normalized);
    }
}
