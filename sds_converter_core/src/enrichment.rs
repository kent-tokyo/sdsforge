/// PubChem CAS lookup and SDS composition enrichment.
///
/// Uses the PubChem PUG REST API to verify CAS numbers and retrieve
/// compound metadata. Network calls are optional — call only when the
/// `--enrich` flag is passed.
use crate::converter::validator::validate_cas_format;
use crate::error::SdsError;
use crate::schema::SdsRoot;

#[derive(Debug, Clone)]
pub struct CasInfo {
    pub cas: String,
    pub iupac_name: Option<String>,
    pub molecular_formula: Option<String>,
    pub pubchem_cid: Option<u64>,
}

#[derive(Debug, Clone)]
pub enum CasWarning {
    NotFound { cas: String },
    NameMismatch { cas: String, pubchem_name: String, sds_name: String },
}

impl std::fmt::Display for CasWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CasWarning::NotFound { cas } => write!(f, "CAS {cas}: not found in PubChem"),
            CasWarning::NameMismatch { cas, pubchem_name, sds_name } => write!(
                f,
                "CAS {cas}: PubChem name '{pubchem_name}' differs from SDS name '{sds_name}'"
            ),
        }
    }
}

/// Look up a CAS number in PubChem and return compound metadata.
///
/// Returns `Ok(None)` when the CAS is not found in PubChem.
/// Returns `Err` only for network or parse errors.
pub async fn lookup_cas(
    cas: &str,
    client: &reqwest::Client,
) -> Result<Option<CasInfo>, SdsError> {
    if !validate_cas_format(cas) {
        return Err(SdsError::Extract(format!("Invalid CAS number: {cas:?}")));
    }
    let url = format!(
        "https://pubchem.ncbi.nlm.nih.gov/rest/pug/compound/name/{cas}/property/IUPACName,MolecularFormula,CID/JSON"
    );
    let mut resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| SdsError::Extract(format!("PubChem request failed: {e}")))?;

    // Retry once after 1 000 ms on HTTP 429 (rate limit).
    if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
        resp = client
            .get(&url)
            .send()
            .await
            .map_err(|e| SdsError::Extract(format!("PubChem request failed (retry): {e}")))?;
    }

    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !resp.status().is_success() {
        return Err(SdsError::Extract(format!(
            "PubChem returned HTTP {}",
            resp.status()
        )));
    }

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| SdsError::Extract(format!("PubChem JSON parse failed: {e}")))?;

    let props = body
        .pointer("/PropertyTable/Properties/0")
        .cloned()
        .unwrap_or(serde_json::Value::Null);

    Ok(Some(CasInfo {
        cas: cas.to_string(),
        iupac_name: props["IUPACName"].as_str().map(str::to_string),
        molecular_formula: props["MolecularFormula"].as_str().map(str::to_string),
        pubchem_cid: props["CID"].as_u64(),
    }))
}

/// Check every CAS number in Section 3 (Composition) against PubChem.
///
/// Returns a list of warnings for CAS numbers not found or whose PubChem
/// IUPAC name differs substantially from the SDS substance name.
pub async fn enrich_composition(
    sds: &SdsRoot,
    client: &reqwest::Client,
) -> Vec<CasWarning> {
    let mut warnings = Vec::new();
    let items = sds
        .composition
        .as_ref()
        .and_then(|c| c.composition_and_concentration.as_deref())
        .unwrap_or(&[]);

    for item in items {
        // Prefer IUPAC name, fall back to CAS inventory name or generic name.
        let sds_name = item
            .substance_identifiers
            .as_ref()
            .and_then(|ids| ids.substance_names.as_ref())
            .and_then(|sn| {
                sn.iupac_name
                    .as_deref()
                    .or(sn.cas_inventory_name.as_deref())
                    .or(sn.generic_name.as_deref())
            })
            .unwrap_or("")
            .to_string();

        let cas_numbers: Vec<String> = item
            .substance_identifiers
            .as_ref()
            .and_then(|ids| ids.substance_identity.as_ref())
            .and_then(|si| si.ca_sno.as_ref())
            .and_then(|c| c.full_text.as_deref())
            .unwrap_or(&[])
            .to_vec();

        for cas in &cas_numbers {
            match lookup_cas(cas, client).await {
                Ok(None) => warnings.push(CasWarning::NotFound { cas: cas.clone() }),
                Ok(Some(info)) => {
                    if let Some(pubchem_name) = &info.iupac_name {
                        if !sds_name.is_empty()
                            && !names_similar(pubchem_name, &sds_name)
                        {
                            warnings.push(CasWarning::NameMismatch {
                                cas: cas.clone(),
                                pubchem_name: pubchem_name.clone(),
                                sds_name: sds_name.clone(),
                            });
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("PubChem lookup error for CAS {cas}: {e}");
                }
            }
            // Rate-limit: 250 ms between PubChem requests to avoid HTTP 429.
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        }
    }

    warnings
}

/// Rough similarity check: true if either name contains the other (case-insensitive).
fn names_similar(a: &str, b: &str) -> bool {
    let a_lo = a.to_lowercase();
    let b_lo = b.to_lowercase();
    a_lo.contains(&b_lo) || b_lo.contains(&a_lo)
}
