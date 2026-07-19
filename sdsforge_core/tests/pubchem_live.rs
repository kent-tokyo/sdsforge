//! Live PubChem contract smoke test.
//!
//! Verifies the actual PUG REST contract `sdsforge_core::enrichment::
//! lookup_cas_detailed` depends on -- the two-step CID resolution flow and
//! the `SMILES`/`ConnectivitySMILES` property names. Requires live network
//! access to pubchem.ncbi.nlm.nih.gov, so it is `#[ignore]`d and never runs
//! in the normal offline CI suite. Run manually before any release that
//! touches PubChem enrichment:
//!
//!   cargo test -p sdsforge-core --features chematic-normalization \
//!     --test pubchem_live -- --ignored --nocapture

use sdsforge_core::{lookup_cas_detailed, CasResolution};

#[cfg(feature = "chematic-normalization")]
use sdsforge_core::{ChematicNormalizer, ChemicalNormalizer};

const SMOKE_TEST_CAS: &[(&str, &str)] = &[
    ("64-17-5", "ethanol"),
    ("7647-14-5", "sodium chloride"),
    ("50-78-2", "aspirin"),
];

#[tokio::test]
#[ignore = "requires live network access to pubchem.ncbi.nlm.nih.gov"]
async fn live_pubchem_contract_smoke_test() {
    let client = reqwest::Client::new();

    for (cas, label) in SMOKE_TEST_CAS {
        let result = lookup_cas_detailed(cas, &client).await.unwrap_or_else(|e| {
            panic!("{label} ({cas}): lookup failed (expected no HTTP 400): {e}")
        });

        let candidate = match result {
            CasResolution::Resolved(c) => c,
            CasResolution::Ambiguous(mut candidates) => candidates.remove(0),
            CasResolution::NotFound => {
                panic!("{label} ({cas}): expected at least one candidate, got NotFound")
            }
        };

        assert!(
            candidate.pubchem_cid.is_some(),
            "{label} ({cas}): missing CID"
        );
        assert!(
            candidate.molecular_formula.is_some(),
            "{label} ({cas}): missing molecular formula"
        );
        assert!(
            candidate.smiles.is_some() || candidate.connectivity_smiles.is_some(),
            "{label} ({cas}): no SMILES representation present"
        );

        #[cfg(feature = "chematic-normalization")]
        {
            let _ = ChematicNormalizer.normalize(&candidate); // must not panic
        }

        println!(
            "{label} ({cas}): CID={:?} formula={:?} smiles={:?} connectivity_smiles={:?}",
            candidate.pubchem_cid,
            candidate.molecular_formula,
            candidate.smiles,
            candidate.connectivity_smiles
        );
    }
}
