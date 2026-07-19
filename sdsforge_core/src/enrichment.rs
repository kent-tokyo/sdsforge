/// PubChem CAS lookup and SDS composition enrichment.
///
/// Uses the PubChem PUG REST API to verify CAS numbers and retrieve
/// compound metadata. Network calls are optional — call only when the
/// `--enrich` flag is passed.
use std::collections::{HashMap, HashSet};
use std::time::Duration;

use crate::converter::validator::validate_cas_format;
use crate::error::SdsError;
use crate::schema::SdsRoot;

const PUBCHEM_BASE_URL: &str = "https://pubchem.ncbi.nlm.nih.gov/rest/pug";

/// PubChem asks clients not to exceed 5 requests/second. Applied before
/// every outbound request in the detailed-resolution path (which needs two
/// requests per CAS), so the aggregate rate stays under the limit
/// regardless of how tightly a caller loops over CAS numbers.
const MIN_REQUEST_INTERVAL: Duration = Duration::from_millis(210);

/// Retries HTTP 429/503 only, with bounded exponential backoff. All other
/// non-success statuses (including 400, which is not retryable) fall
/// through immediately.
const MAX_RETRIES: u32 = 3;
const BASE_BACKOFF: Duration = Duration::from_millis(250);
const MAX_RETRY_AFTER: Duration = Duration::from_secs(30);

/// Cap on the PubChem fault detail included in an error message — never
/// embed an arbitrary (potentially large, potentially HTML) response body
/// in full.
const MAX_FAULT_MESSAGE_LEN: usize = 2048;

#[derive(Debug, Clone)]
pub struct CasInfo {
    pub cas: String,
    pub iupac_name: Option<String>,
    pub molecular_formula: Option<String>,
    pub pubchem_cid: Option<u64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum CasWarning {
    NotFound {
        cas: String,
    },
    NameMismatch {
        cas: String,
        pubchem_name: String,
        sds_name: String,
    },
}

impl std::fmt::Display for CasWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CasWarning::NotFound { cas } => write!(f, "CAS {cas}: not found in PubChem"),
            CasWarning::NameMismatch {
                cas,
                pubchem_name,
                sds_name,
            } => write!(
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
///
/// Unchanged since before the detailed-resolution rewrite: requests only
/// `IUPACName,MolecularFormula,CID` — none of the obsolete SMILES property
/// names `lookup_cas_detailed` used to request — so it never hit the
/// PubChem 400 this fix addresses, and existing callers keep their exact
/// current behavior.
pub async fn lookup_cas(cas: &str, client: &reqwest::Client) -> Result<Option<CasInfo>, SdsError> {
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
pub async fn enrich_composition(sds: &SdsRoot, client: &reqwest::Client) -> Vec<CasWarning> {
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
                        if !sds_name.is_empty() && !names_similar(pubchem_name, &sds_name) {
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

/// A single chemical identity candidate PubChem returned for a CAS number.
/// Distinct from [`CasInfo`] (which assumes exactly one match, per
/// `lookup_cas`'s existing behavior) — this carries structure fields too,
/// and [`CasResolution`] makes ">1 candidate" representable instead of
/// silently discarding all but the first.
#[derive(Debug, Clone)]
pub struct ChemicalIdentityCandidate {
    pub cas: String,
    pub pubchem_cid: Option<u64>,
    pub iupac_name: Option<String>,
    pub molecular_formula: Option<String>,
    /// PubChem's `SMILES` property — includes stereochemistry and isotopes
    /// where PubChem represents them. The resolver's own value, kept
    /// separate from anything a normalizer later derives from it. Not to be
    /// confused with a *canonical* SMILES — chematic's locally-computed
    /// canonical form is a distinct, separately-provenanced value (see
    /// `generation::provenance::FieldProvenance::canonical_smiles`).
    pub smiles: Option<String>,
    /// PubChem's `ConnectivitySMILES` property — connectivity only, no
    /// stereochemistry/isotope information. Used as a normalization-input
    /// fallback only when `smiles` is unavailable (see
    /// `ChematicNormalizer::normalize`), since it represents strictly less
    /// of the actual structure.
    pub connectivity_smiles: Option<String>,
    pub inchi_key: Option<String>,
}

/// Result of resolving one CAS number against PubChem, without silently
/// collapsing multiple matches to the first one the way `lookup_cas` does.
#[derive(Debug, Clone)]
pub enum CasResolution {
    Resolved(ChemicalIdentityCandidate),
    NotFound,
    /// PubChem's name-match returned more than one distinct CID for this
    /// CAS. A material identity ambiguity — callers must not pick one
    /// automatically (by name similarity, CID order, or any other
    /// heuristic); see `generation`'s `AmbiguousChemicalIdentity` handling.
    Ambiguous(Vec<ChemicalIdentityCandidate>),
}

/// Deduplicates a PubChem CID list (`/IdentifierList/CID`) while preserving
/// the order PubChem returned them in — pure, no network, testable with
/// synthetic JSON.
fn parse_cid_list(body: &serde_json::Value) -> Vec<u64> {
    let mut seen = HashSet::new();
    body.pointer("/IdentifierList/CID")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_u64())
                .filter(|cid| seen.insert(*cid))
                .collect()
        })
        .unwrap_or_default()
}

/// Parses a `compound/cid/{cids}/property/...` response into one candidate
/// per row, keyed by each row's own `CID` field (not by request position —
/// PubChem does not guarantee the response preserves request order).
fn parse_candidates_by_cid(
    cas: &str,
    body: &serde_json::Value,
) -> HashMap<u64, ChemicalIdentityCandidate> {
    body.pointer("/PropertyTable/Properties")
        .and_then(|v| v.as_array())
        .map(|props| {
            props
                .iter()
                .filter_map(|p| {
                    let cid = p["CID"].as_u64()?;
                    Some((cid, candidate_from_properties(cas, cid, p)))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn candidate_from_properties(
    cas: &str,
    cid: u64,
    props: &serde_json::Value,
) -> ChemicalIdentityCandidate {
    ChemicalIdentityCandidate {
        cas: cas.to_string(),
        pubchem_cid: Some(cid),
        iupac_name: props["IUPACName"].as_str().map(str::to_string),
        molecular_formula: props["MolecularFormula"].as_str().map(str::to_string),
        smiles: props["SMILES"].as_str().map(str::to_string),
        connectivity_smiles: props["ConnectivitySMILES"].as_str().map(str::to_string),
        inchi_key: props["InChIKey"].as_str().map(str::to_string),
    }
}

/// Builds the final [`CasResolution`] from a deduplicated exact-match CID
/// list and the per-CID property lookup. Classification is driven by
/// `cids.len()` — the exact-synonym CID count from PubChem's own `cids`
/// endpoint — never by how many property rows happened to come back, so
/// "0/1/2+ CIDs" maps directly to `NotFound`/`Resolved`/`Ambiguous`. A CID
/// present in the list but inexplicably missing from the property response
/// still produces a (mostly-empty) candidate rather than silently
/// vanishing, so the CID count and candidate count always agree.
fn build_resolution(
    cas: &str,
    cids: &[u64],
    mut properties_by_cid: HashMap<u64, ChemicalIdentityCandidate>,
) -> CasResolution {
    let candidates: Vec<ChemicalIdentityCandidate> = cids
        .iter()
        .map(|cid| {
            properties_by_cid
                .remove(cid)
                .unwrap_or_else(|| ChemicalIdentityCandidate {
                    cas: cas.to_string(),
                    pubchem_cid: Some(*cid),
                    iupac_name: None,
                    molecular_formula: None,
                    smiles: None,
                    connectivity_smiles: None,
                    inchi_key: None,
                })
        })
        .collect();

    match candidates.len() {
        0 => CasResolution::NotFound,
        1 => CasResolution::Resolved(candidates.into_iter().next().expect("len checked above")),
        _ => CasResolution::Ambiguous(candidates),
    }
}

/// Truncates `s` to at most `max_bytes` bytes on a UTF-8 char boundary,
/// appending `...` if truncated.
fn truncate_bounded(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &s[..end])
}

/// Formats a bounded, sanitized diagnostic from a non-success PubChem
/// response body. Prefers the structured `Fault.Message`/`Fault.Details`
/// fields PUG REST returns for most error responses; falls back to a
/// truncated raw body for anything else (e.g. an HTML error page) rather
/// than including it in full. Pure — takes the already-fetched body text,
/// not a live `Response`, so it's testable without HTTP.
fn format_pubchem_fault(status: reqwest::StatusCode, body: &str) -> String {
    let detail = serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| {
            let message = v
                .pointer("/Fault/Message")
                .and_then(|m| m.as_str())
                .map(str::to_string);
            let details = v
                .pointer("/Fault/Details")
                .and_then(|d| d.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|x| x.as_str())
                        .collect::<Vec<_>>()
                        .join("; ")
                });
            match (message, details) {
                (Some(m), Some(d)) if !d.is_empty() => Some(format!("{m} — {d}")),
                (Some(m), _) => Some(m),
                (None, Some(d)) if !d.is_empty() => Some(d),
                _ => None,
            }
        })
        .unwrap_or_else(|| body.to_string());

    format!(
        "PubChem returned HTTP {status}: {}",
        truncate_bounded(&detail, MAX_FAULT_MESSAGE_LEN)
    )
}

async fn describe_pubchem_error(status: reqwest::StatusCode, resp: reqwest::Response) -> SdsError {
    let body = resp.text().await.unwrap_or_default();
    SdsError::Extract(format_pubchem_fault(status, &body))
}

/// Waits out `Retry-After` (seconds, capped) if PubChem sent one; otherwise
/// `None`, and the caller falls back to its own backoff.
fn retry_after_duration(resp: &reqwest::Response) -> Option<Duration> {
    resp.headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .map(|secs| Duration::from_secs(secs).min(MAX_RETRY_AFTER))
}

/// Issues a GET, retrying only HTTP 429/503 with bounded backoff (honoring
/// `Retry-After` when PubChem sends one). Every attempt — including the
/// first — is preceded by [`MIN_REQUEST_INTERVAL`], so this bounds the
/// aggregate PubChem request rate regardless of caller loop behavior.
/// Returns whatever the final response was (success, a non-retryable
/// error, or the last retryable error after the retry cap) — response
/// interpretation (404 vs. other errors vs. success) is the caller's job.
async fn get_with_retry(
    client: &reqwest::Client,
    url: &str,
) -> Result<reqwest::Response, SdsError> {
    let mut attempt = 0;
    loop {
        tokio::time::sleep(MIN_REQUEST_INTERVAL).await;
        let resp = client
            .get(url)
            .send()
            .await
            .map_err(|e| SdsError::Extract(format!("PubChem request failed: {e}")))?;

        let status = resp.status();
        let retryable = status == reqwest::StatusCode::TOO_MANY_REQUESTS
            || status == reqwest::StatusCode::SERVICE_UNAVAILABLE;

        if !retryable || attempt >= MAX_RETRIES {
            return Ok(resp);
        }

        let delay = retry_after_duration(&resp).unwrap_or_else(|| BASE_BACKOFF * 2u32.pow(attempt));
        tokio::time::sleep(delay).await;
        attempt += 1;
    }
}

async fn resolve_cids(
    cas: &str,
    client: &reqwest::Client,
    base_url: &str,
) -> Result<Vec<u64>, SdsError> {
    let url = format!("{base_url}/compound/name/{cas}/cids/JSON?name_type=complete");
    let resp = get_with_retry(client, &url).await?;

    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(Vec::new());
    }
    if !resp.status().is_success() {
        let status = resp.status();
        return Err(describe_pubchem_error(status, resp).await);
    }

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| SdsError::Extract(format!("PubChem JSON parse failed: {e}")))?;
    Ok(parse_cid_list(&body))
}

async fn fetch_properties_by_cid(
    cas: &str,
    cids: &[u64],
    client: &reqwest::Client,
    base_url: &str,
) -> Result<HashMap<u64, ChemicalIdentityCandidate>, SdsError> {
    let cid_list = cids
        .iter()
        .map(u64::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let url = format!(
        "{base_url}/compound/cid/{cid_list}/property/IUPACName,MolecularFormula,SMILES,ConnectivitySMILES,InChIKey/JSON"
    );
    let resp = get_with_retry(client, &url).await?;

    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(HashMap::new());
    }
    if !resp.status().is_success() {
        let status = resp.status();
        return Err(describe_pubchem_error(status, resp).await);
    }

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| SdsError::Extract(format!("PubChem JSON parse failed: {e}")))?;
    Ok(parse_candidates_by_cid(cas, &body))
}

/// Like [`lookup_cas`], but never silently discards additional PubChem
/// candidates — returns [`CasResolution::Ambiguous`] instead of picking the
/// first. Two-step exact resolution, not a single combined
/// `compound/name/{cas}/property/...` request: step 1 resolves the CAS
/// synonym to its exact-match CID list (`compound/name/{cas}/cids/JSON?
/// name_type=complete`); step 2 retrieves properties for that CID list
/// (`compound/cid/{cids}/property/...`). A name lookup can map to more than
/// one CID, and PubChem's combined name+property endpoint does not
/// guarantee it surfaces all of them the same way the dedicated `cids`
/// endpoint does — this is also what fixed the HTTP 400 the old single-step
/// request produced by requesting the obsolete `CanonicalSMILES`/
/// `IsomericSMILES` property names (current PUG REST uses `SMILES`/
/// `ConnectivitySMILES`; `CID` is never requested as a property, since
/// every property row already carries its own `CID`).
///
/// `lookup_cas` itself is unchanged — it requests none of the property
/// names this fix touches, so it never needed a two-step rewrite.
pub async fn lookup_cas_detailed(
    cas: &str,
    client: &reqwest::Client,
) -> Result<CasResolution, SdsError> {
    lookup_cas_detailed_at(cas, client, PUBCHEM_BASE_URL).await
}

async fn lookup_cas_detailed_at(
    cas: &str,
    client: &reqwest::Client,
    base_url: &str,
) -> Result<CasResolution, SdsError> {
    if !validate_cas_format(cas) {
        return Err(SdsError::Extract(format!("Invalid CAS number: {cas:?}")));
    }

    let cids = resolve_cids(cas, client, base_url).await?;
    if cids.is_empty() {
        return Ok(CasResolution::NotFound);
    }

    let properties_by_cid = fetch_properties_by_cid(cas, &cids, client, base_url).await?;
    Ok(build_resolution(cas, &cids, properties_by_cid))
}

/// Word-level Jaccard similarity check (threshold ≥ 0.5, case-insensitive).
///
/// The old substring approach produced false positives for short common words
/// (e.g. "acid" matching "acetic acid") and was O(n²) on long names.
/// Jaccard on word tokens is both more precise and still O(n).
fn names_similar(a: &str, b: &str) -> bool {
    let a_lo = a.to_lowercase();
    let b_lo = b.to_lowercase();
    let words_a: HashSet<&str> = a_lo.split_whitespace().collect();
    let words_b: HashSet<&str> = b_lo.split_whitespace().collect();
    if words_a.is_empty() || words_b.is_empty() {
        return false;
    }
    let intersection = words_a.intersection(&words_b).count();
    let union = words_a.union(&words_b).count();
    (intersection as f64 / union as f64) >= 0.5
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn names_similar_identical() {
        assert!(names_similar("acetic acid", "acetic acid"));
    }

    #[test]
    fn names_similar_partial_overlap_above_threshold() {
        assert!(names_similar("acetic acid solution", "acetic acid"));
    }

    #[test]
    fn names_similar_no_overlap() {
        assert!(!names_similar("sodium chloride", "acetic acid"));
    }

    #[test]
    fn names_similar_short_common_word_no_false_positive() {
        assert!(!names_similar("salt", "sodium chloride"));
    }

    #[test]
    fn names_similar_empty_string() {
        assert!(!names_similar("", "acetic acid"));
        assert!(!names_similar("acetic acid", ""));
    }

    // -- pure parsing: parse_cid_list -------------------------------------

    #[test]
    fn parse_cid_list_empty() {
        let body = serde_json::json!({ "IdentifierList": { "CID": [] } });
        assert_eq!(parse_cid_list(&body), Vec::<u64>::new());
    }

    #[test]
    fn parse_cid_list_missing_key() {
        let body = serde_json::json!({});
        assert_eq!(parse_cid_list(&body), Vec::<u64>::new());
    }

    #[test]
    fn parse_cid_list_single() {
        let body = serde_json::json!({ "IdentifierList": { "CID": [702] } });
        assert_eq!(parse_cid_list(&body), vec![702]);
    }

    #[test]
    fn parse_cid_list_deduplicates_preserving_order() {
        let body = serde_json::json!({ "IdentifierList": { "CID": [5, 3, 5, 1, 3] } });
        assert_eq!(parse_cid_list(&body), vec![5, 3, 1]);
    }

    // -- pure parsing: parse_candidates_by_cid / candidate_from_properties -

    #[test]
    fn candidate_parsing_maps_smiles() {
        let body = serde_json::json!({
            "PropertyTable": { "Properties": [
                {"CID": 702, "SMILES": "CCO", "IUPACName": "ethanol"}
            ] }
        });
        let by_cid = parse_candidates_by_cid("64-17-5", &body);
        assert_eq!(by_cid[&702].smiles.as_deref(), Some("CCO"));
    }

    #[test]
    fn candidate_parsing_maps_connectivity_smiles() {
        let body = serde_json::json!({
            "PropertyTable": { "Properties": [
                {"CID": 702, "ConnectivitySMILES": "CCO"}
            ] }
        });
        let by_cid = parse_candidates_by_cid("64-17-5", &body);
        assert_eq!(by_cid[&702].connectivity_smiles.as_deref(), Some("CCO"));
    }

    // -- pure: build_resolution --------------------------------------------

    #[test]
    fn build_resolution_zero_cids_is_not_found() {
        let resolution = build_resolution("0-00-0", &[], HashMap::new());
        assert!(matches!(resolution, CasResolution::NotFound));
    }

    #[test]
    fn build_resolution_one_cid_is_resolved() {
        let mut props = HashMap::new();
        props.insert(
            702,
            candidate_from_properties(
                "64-17-5",
                702,
                &serde_json::json!({"IUPACName": "ethanol", "SMILES": "CCO"}),
            ),
        );
        let resolution = build_resolution("64-17-5", &[702], props);
        match resolution {
            CasResolution::Resolved(c) => {
                assert_eq!(c.pubchem_cid, Some(702));
                assert_eq!(c.iupac_name.as_deref(), Some("ethanol"));
            }
            other => panic!("expected Resolved, got {other:?}"),
        }
    }

    #[test]
    fn build_resolution_two_cids_is_ambiguous_preserving_all_candidates() {
        let mut props = HashMap::new();
        props.insert(
            1,
            candidate_from_properties("64-17-5", 1, &serde_json::json!({})),
        );
        props.insert(
            2,
            candidate_from_properties("64-17-5", 2, &serde_json::json!({})),
        );
        let resolution = build_resolution("64-17-5", &[1, 2], props);
        match resolution {
            CasResolution::Ambiguous(candidates) => {
                assert_eq!(candidates.len(), 2);
                let cids: Vec<Option<u64>> = candidates.iter().map(|c| c.pubchem_cid).collect();
                assert_eq!(cids, vec![Some(1), Some(2)]);
            }
            other => panic!("expected Ambiguous, got {other:?}"),
        }
    }

    // -- pure: format_pubchem_fault / truncate_bounded ----------------------

    #[test]
    fn fault_message_prefers_structured_fault_fields() {
        let body = serde_json::json!({
            "Fault": {"Code": "PUGREST.BadRequest", "Message": "Invalid property"}
        })
        .to_string();
        let msg = format_pubchem_fault(reqwest::StatusCode::BAD_REQUEST, &body);
        assert!(msg.contains("Invalid property"));
        assert!(msg.contains("400"));
    }

    #[test]
    fn fault_message_includes_details_when_present() {
        let body = serde_json::json!({
            "Fault": {
                "Code": "PUGREST.NotFound",
                "Message": "No CID found",
                "Details": ["No CID found that matches the given name"]
            }
        })
        .to_string();
        let msg = format_pubchem_fault(reqwest::StatusCode::NOT_FOUND, &body);
        assert!(msg.contains("No CID found"));
        assert!(msg.contains("matches the given name"));
    }

    #[test]
    fn fault_message_is_bounded() {
        let huge = "<html>".to_string() + &"x".repeat(10_000) + "</html>";
        let msg = format_pubchem_fault(reqwest::StatusCode::BAD_REQUEST, &huge);
        assert!(msg.len() < MAX_FAULT_MESSAGE_LEN + 100);
    }

    #[test]
    fn truncate_bounded_does_not_panic_on_multibyte_boundary() {
        // Each "あ" is 3 bytes in UTF-8; a naive byte-index slice at an odd
        // boundary would panic.
        let s = "あ".repeat(1000);
        let truncated = truncate_bounded(&s, 10);
        assert!(truncated.len() <= 13); // 10 + "..."
    }

    // -- HTTP-mocked: request shape, ambiguity, retry/rate-limit -----------

    #[tokio::test]
    async fn request_uses_smiles_and_connectivity_smiles_not_obsolete_names() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/compound/name/64-17-5/cids/JSON"))
            .and(query_param("name_type", "complete"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "IdentifierList": { "CID": [702] }
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/compound/cid/702/property/IUPACName,MolecularFormula,SMILES,ConnectivitySMILES,InChIKey/JSON"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "PropertyTable": { "Properties": [
                    {"CID": 702, "IUPACName": "ethanol", "MolecularFormula": "C2H6O",
                     "SMILES": "CCO", "ConnectivitySMILES": "CCO", "InChIKey": "LFQSCWFLJHTTHZ-UHFFFAOYSA-N"}
                ] }
            })))
            .expect(1)
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let result = lookup_cas_detailed_at("64-17-5", &client, &server.uri()).await;
        assert!(matches!(result, Ok(CasResolution::Resolved(_))));
        // wiremock's .expect(1) on each Mock asserts on drop that exactly
        // one matching request (with this exact path/property list) was
        // received -- proving CanonicalSMILES/IsomericSMILES/CID-as-property
        // were never requested, since a mismatched request would 404
        // against these mocks instead of matching.
    }

    #[tokio::test]
    async fn zero_cids_is_not_found_without_a_second_request() {
        let server = MockServer::start().await;
        // A well-formed, valid-check-digit CAS (aspirin) that simply isn't
        // found -- distinct from a malformed CAS, which is rejected before
        // any request is ever made.
        Mock::given(method("GET"))
            .and(path("/compound/name/50-78-2/cids/JSON"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "IdentifierList": { "CID": [] }
            })))
            .expect(1)
            .mount(&server)
            .await;
        // No mock registered for /compound/cid/... -- if the code ever
        // called it, wiremock would return its default 404 and the
        // resolution would incorrectly become NotFound-via-error instead
        // of NotFound-via-empty-CID-list, but the real assertion is the
        // request count captured below.

        let client = reqwest::Client::new();
        let result = lookup_cas_detailed_at("50-78-2", &client, &server.uri()).await;
        assert!(matches!(result, Ok(CasResolution::NotFound)));
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn duplicate_cids_are_deduplicated_before_the_property_request() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/compound/name/64-17-5/cids/JSON"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "IdentifierList": { "CID": [702, 702] }
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/compound/cid/702/property/IUPACName,MolecularFormula,SMILES,ConnectivitySMILES,InChIKey/JSON"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "PropertyTable": { "Properties": [{"CID": 702, "IUPACName": "ethanol"}] }
            })))
            .expect(1)
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let result = lookup_cas_detailed_at("64-17-5", &client, &server.uri()).await;
        assert!(matches!(result, Ok(CasResolution::Resolved(_))));
    }

    #[tokio::test]
    async fn multiple_distinct_cids_yield_ambiguous() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/compound/name/64-17-5/cids/JSON"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "IdentifierList": { "CID": [1, 2] }
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/compound/cid/1,2/property/IUPACName,MolecularFormula,SMILES,ConnectivitySMILES,InChIKey/JSON"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "PropertyTable": { "Properties": [
                    {"CID": 1, "IUPACName": "candidate one"},
                    {"CID": 2, "IUPACName": "candidate two"}
                ] }
            })))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let result = lookup_cas_detailed_at("64-17-5", &client, &server.uri()).await;
        match result {
            Ok(CasResolution::Ambiguous(candidates)) => assert_eq!(candidates.len(), 2),
            other => panic!("expected Ambiguous, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn http_400_includes_bounded_fault_detail_and_is_not_retried() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/compound/name/64-17-5/cids/JSON"))
            .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
                "Fault": {"Code": "PUGREST.BadRequest", "Message": "Invalid property"}
            })))
            .expect(1)
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let result = lookup_cas_detailed_at("64-17-5", &client, &server.uri()).await;
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Invalid property"));
        assert!(err.contains("400"));
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn http_429_is_retried_with_backoff() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/compound/name/64-17-5/cids/JSON"))
            .respond_with(ResponseTemplate::new(429))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/compound/name/64-17-5/cids/JSON"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "IdentifierList": { "CID": [] }
            })))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let result = lookup_cas_detailed_at("64-17-5", &client, &server.uri()).await;
        assert!(matches!(result, Ok(CasResolution::NotFound)));
        assert_eq!(server.received_requests().await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn http_503_is_retried_with_backoff() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/compound/name/64-17-5/cids/JSON"))
            .respond_with(ResponseTemplate::new(503))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/compound/name/64-17-5/cids/JSON"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "IdentifierList": { "CID": [] }
            })))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let result = lookup_cas_detailed_at("64-17-5", &client, &server.uri()).await;
        assert!(matches!(result, Ok(CasResolution::NotFound)));
        assert_eq!(server.received_requests().await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn retries_are_capped() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/compound/name/64-17-5/cids/JSON"))
            .respond_with(ResponseTemplate::new(429))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let result = lookup_cas_detailed_at("64-17-5", &client, &server.uri()).await;
        assert!(result.is_err());
        // Initial attempt + MAX_RETRIES retries, never more.
        assert_eq!(
            server.received_requests().await.unwrap().len() as u32,
            MAX_RETRIES + 1
        );
    }

    #[tokio::test]
    async fn a_failed_cas_does_not_prevent_processing_a_later_component() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/compound/name/bad-cas/cids/JSON"))
            .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
                "Fault": {"Code": "PUGREST.BadRequest", "Message": "boom"}
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/compound/name/64-17-5/cids/JSON"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "IdentifierList": { "CID": [702] }
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/compound/cid/702/property/IUPACName,MolecularFormula,SMILES,ConnectivitySMILES,InChIKey/JSON"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "PropertyTable": { "Properties": [{"CID": 702, "IUPACName": "ethanol"}] }
            })))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        // "bad-cas" isn't a valid CAS format, so this exercises the format
        // guard rather than the mock -- the real point is that the second,
        // independent call for a valid CAS still succeeds regardless of
        // what happened to the first.
        let first = lookup_cas_detailed_at("bad-cas", &client, &server.uri()).await;
        assert!(first.is_err());
        let second = lookup_cas_detailed_at("64-17-5", &client, &server.uri()).await;
        assert!(matches!(second, Ok(CasResolution::Resolved(_))));
    }
}
