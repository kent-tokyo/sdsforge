//! Golden acceptance test for the salt/multi-fragment scenario, part of
//! `sdsforge generate`'s golden-test suite (see also
//! `sdsforge/tests/generate_golden.rs` for the three offline CLI-level
//! scenarios).
//!
//! Sodium chloride only exercises chematic normalization after a
//! PubChem-resolved candidate — reachable in production only via
//! `--enrich`. Rather than mocking PubChem at the CLI layer (which has no
//! test seam for that today), this calls `generate_from_normalized_input`
//! directly with a hand-built `ChemicalIdentityCandidate` — the exact same
//! production function `--enrich` calls internally, just without the HTTP
//! hop. Fully offline, no network, no CLI subprocess.

#![cfg(feature = "chematic-normalization")]

use std::collections::{BTreeSet, HashMap};

use sdsforge_core::generation::field_path;
use sdsforge_core::{
    build_generation_artifacts, generate_from_normalized_input, validate_typed, CasResolution,
    ChematicNormalizer, ChemicalIdentityCandidate, ComponentInput, ConcentrationRange,
    EvidenceLevel, ProductInput, SdsRoot, SupplierInput,
};

fn product() -> ProductInput {
    ProductInput {
        trade_name: "Golden Fixture Salt".into(),
        other_names: vec![],
        supplier: SupplierInput {
            company_name: "Golden Fixture Chemical Co., Ltd.".into(),
            address: None,
            phone: None,
            email: None,
        },
        components: vec![ComponentInput {
            cas_number: Some("7647-14-5".into()),
            name: Some("Sodium Chloride".into()),
            concentration: ConcentrationRange {
                exact: Some(100.0),
                lower: None,
                upper: None,
                unit: "%".into(),
            },
        }],
        measured_properties: Default::default(),
        evidence: vec![],
    }
}

fn resolved_sodium_chloride() -> HashMap<String, CasResolution> {
    let candidate = ChemicalIdentityCandidate {
        cas: "7647-14-5".into(),
        pubchem_cid: Some(5234),
        iupac_name: Some("sodium chloride".into()),
        molecular_formula: Some("ClNa".into()),
        smiles: Some("[Na+].[Cl-]".into()),
        connectivity_smiles: Some("[Na+].[Cl-]".into()),
        inchi_key: None,
    };
    let mut resolved = HashMap::new();
    resolved.insert("7647-14-5".to_string(), CasResolution::Resolved(candidate));
    resolved
}

#[test]
fn salt_sodium_chloride_matches_golden() {
    let result = generate_from_normalized_input(
        &product(),
        &resolved_sodium_chloride(),
        &ChematicNormalizer,
    );
    let artifacts = build_generation_artifacts(&result).unwrap();

    let actual: serde_json::Value = serde_json::from_str(&artifacts.official_sds_json).unwrap();
    let expected: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/generate_golden/salt_sodium_chloride.expected_official_sds.json"
        ))
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        actual, expected,
        "official_sds.json does not match the golden fixture"
    );

    // Both ions remain -- never silently reduced to a single largest
    // fragment.
    assert!(
        actual["Composition"]["CompositionAndConcentration"][0]["SMILES"]
            .as_str()
            .unwrap()
            .contains('.')
    );

    // generate only ever populates Identification/Composition (no
    // PhysicalChemicalProperties -- no measured-property evidence was
    // supplied) -- Section 2 and every other not-yet-implemented section
    // must never be fabricated.
    let keys: BTreeSet<&str> = actual
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys, BTreeSet::from(["Composition", "Identification"]));

    // Multi-fragment alone is a MED finding, not blocking.
    assert_eq!(
        result.release_status,
        sdsforge_core::ReleaseStatus::ReviewRequired
    );
    assert!(artifacts.review_report_markdown.contains("REVIEW REQUIRED"));
    assert!(artifacts
        .review_report_markdown
        .contains(&format!("Unresolved fields: {}", result.unresolved.len())));

    // Properties nobody supplied evidence for stay explicitly unresolved --
    // same set as the single-substance scenario, minus CASno (resolved here
    // via the hand-built PubChem candidate, unlike the offline CLI scenarios).
    let unresolved_paths: BTreeSet<&str> =
        result.unresolved.iter().map(|u| u.path.as_str()).collect();
    assert_eq!(
        unresolved_paths,
        BTreeSet::from([
            "HazardIdentification.Classification.PhysicochemicalEffect.CorrosiveToMetals",
            "PhysicalChemicalProperties.ExplosiveLimits",
            "PhysicalChemicalProperties.FlashPoint",
            "PhysicalChemicalProperties.InitialBoilingPointAndBoilingRange",
            "PhysicalChemicalProperties.OxidizingProperties",
            "PhysicalChemicalProperties.VapourPressure",
            "StabilityReactivity.SelfReactivity",
        ])
    );

    // Positive-inclusion check: prove the multi-fragment code path
    // actually ran, so this test can't pass by accident.
    assert!(result
        .findings
        .iter()
        .any(|f| f.rule == "GEN-STRUCTURE-MULTIFRAGMENT" && f.level == "MED"));
    // ... and nothing HIGH or above -- multi-fragment never blocks release
    // by itself.
    assert!(result
        .findings
        .iter()
        .all(|f| f.level != "HIGH" && f.level != "CRIT"));

    // validate_typed's own MHLW-conformance rule catalog: CRIT must always
    // be zero, and (a different rule catalog from generate's own findings
    // above) it shows no HIGH for this scenario either.
    let sds: SdsRoot = serde_json::from_value(actual.clone()).unwrap();
    let conformance = validate_typed(&sds);
    assert!(
        conformance.iter().all(|f| f.level != "CRIT"),
        "unexpected CRIT findings: {conformance:#?}"
    );
    assert!(conformance.iter().all(|f| f.level != "HIGH"));

    // Provenance spot-check: chematic-derived structure fields must be
    // attributed to how they were actually obtained (a reference-database
    // lookup, then a deterministic recomputation) -- never claimed at a
    // product-test-evidence level nobody supplied.
    let source_smiles_provenance = result
        .provenance
        .iter()
        .find(|p| p.path == field_path::SOURCE_SMILES)
        .expect("SourceSmiles provenance entry missing");
    assert_eq!(
        source_smiles_provenance.source_type,
        EvidenceLevel::ReferenceDatabase
    );
    let canonical_smiles_provenance = result
        .provenance
        .iter()
        .find(|p| p.path == field_path::CANONICAL_SMILES)
        .expect("canonical SMILES provenance entry missing");
    assert_eq!(
        canonical_smiles_provenance.source_type,
        EvidenceLevel::DeterministicCalculation
    );
    for p in [source_smiles_provenance, canonical_smiles_provenance] {
        assert!(
            !matches!(
                p.source_type,
                EvidenceLevel::ProductTestReport | EvidenceLevel::EquivalentBatchTestReport
            ),
            "{} must never claim product-test-level evidence nobody supplied",
            p.path
        );
    }
}
