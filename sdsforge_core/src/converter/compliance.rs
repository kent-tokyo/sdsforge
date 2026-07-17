//! Country-specific SDS compliance gap analysis.
//!
//! [`generate_compliance_diff`] inspects a converted [`SdsRoot`] and checks it
//! against the mandatory requirements of a specific national SDS standard,
//! returning a [`ComplianceDiffReport`] that lists every gap with severity and
//! a human-readable recommendation.
//!
//! The report is included in [`super::ConversionReport::compliance_diff`] and is
//! also written as a companion `<stem>_compliance_<country>.json` file by the CLI.

use serde::{Deserialize, Serialize};

use crate::country::SourceCountry;
use crate::schema::SdsRoot;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// One identified gap between the converted SDS and a national regulatory requirement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceGap {
    /// SDS section number (1–16).
    pub section: u8,
    /// Dot-path to the affected field in the MHLW JSON schema.
    pub field: String,
    /// Regulatory severity: `"Critical"`, `"Major"`, or `"Advisory"`.
    pub severity: String,
    /// Human-readable description of what the standard requires.
    pub requirement: String,
    /// Actionable suggestion for how to fill the gap.
    pub recommendation: String,
}

/// Full compliance-gap report for a single target country.
///
/// Serialised as JSON alongside the main SDS JSON output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceDiffReport {
    /// English name of the target country (e.g. `"China"`).
    pub target_country: String,
    /// Primary regulatory standard (e.g. `"GB/T 16483-2008 / GB/T 17519-2013"`).
    pub standard: String,
    /// Number of gaps found (0 = fully compliant with checked rules).
    pub gap_count: usize,
    /// Ordered list of compliance gaps; empty if no gaps detected.
    pub gaps: Vec<ComplianceGap>,
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Generate a compliance-gap report for `sds` against `country`'s regulatory standard.
pub fn generate_compliance_diff(sds: &SdsRoot, country: SourceCountry) -> ComplianceDiffReport {
    let mut gaps: Vec<ComplianceGap> = Vec::new();

    match country {
        SourceCountry::China => check_china(sds, &mut gaps),
        SourceCountry::Korea => check_korea(sds, &mut gaps),
        SourceCountry::Taiwan => check_taiwan(sds, &mut gaps),
        SourceCountry::Japan => check_japan(sds, &mut gaps),
    }

    let gap_count = gaps.len();
    ComplianceDiffReport {
        target_country: country.name_en().into(),
        standard: country.regulatory_standard().into(),
        gap_count,
        gaps,
    }
}

// ---------------------------------------------------------------------------
// Country-specific rule sets
// ---------------------------------------------------------------------------

fn check_china(sds: &SdsRoot, gaps: &mut Vec<ComplianceGap>) {
    // ── Section 1: 24-hour emergency contact ────────────────────────────────
    if !has_emergency_contact(sds) {
        gaps.push(ComplianceGap {
            section: 1,
            field: "Identification.SupplierInformation.EmergencyContact".into(),
            severity: "Critical".into(),
            requirement: "GB/T 16483 §5.1 mandates a 24-hour emergency telephone number \
                          (紧急电话 / 24小时应急电话) on every SDS."
                .into(),
            recommendation: "Add an EmergencyContact entry with Phone set to the 24-hour \
                             emergency line and WorkingHours set to '24小时'."
                .into(),
        });
    }

    // ── Section 8: Chinese occupational exposure limits ─────────────────────
    if !has_exposure_limit_reference(sds, &["GBZ", "职业接触限值", "职业卫生"]) {
        gaps.push(ComplianceGap {
            section: 8,
            field: "ExposureControlPersonalProtection".into(),
            severity: "Major".into(),
            requirement: "GB/T 17519 §8 requires Chinese occupational exposure limits \
                          (职业接触限值) referencing GBZ 2 where values exist."
                .into(),
            recommendation: "Add OEL values from GBZ 2 (Chinese OSHA standard) into \
                             ExposureControlPersonalProtection.OccupationalExposureLimits."
                .into(),
        });
    }

    // ── Section 11: Category 5 acute toxicity ───────────────────────────────
    if !has_acute_toxicity_data(sds) {
        gaps.push(ComplianceGap {
            section: 11,
            field: "ToxicologicalInformation[].AcuteToxicity".into(),
            severity: "Major".into(),
            requirement: "GB/T 16483 requires acute toxicity data (LD50/LC50) for all \
                          relevant exposure routes, including GHS Category 5 \
                          (oral LD50 2000–5000 mg/kg) where applicable."
                .into(),
            recommendation: "Extract and populate AcuteToxicity.ExposureRoute entries for \
                             oral, dermal, and inhalation routes with LD50/LC50 values."
                .into(),
        });
    }

    // ── Section 15: Chinese regulatory references ────────────────────────────
    if !regulatory_mentions_keyword(sds, &["危险化学品", "GB 13690", "GB13690", "GB 30000"]) {
        gaps.push(ComplianceGap {
            section: 15,
            field: "RegulatoryInformation.OtherLegislation".into(),
            severity: "Critical".into(),
            requirement: "GB/T 16483 §15 requires explicit reference to the Hazardous \
                          Chemicals Catalogue (危险化学品目录) and GB 13690 classification."
                .into(),
            recommendation: "Add entries in RegulatoryInformation.OtherLegislation \
                             referencing 危险化学品目录 and GB 13690/GB 30000 series."
                .into(),
        });
    }
}

fn check_korea(sds: &SdsRoot, gaps: &mut Vec<ComplianceGap>) {
    // ── Section 1: emergency contact ────────────────────────────────────────
    if !has_emergency_contact(sds) {
        gaps.push(ComplianceGap {
            section: 1,
            field: "Identification.SupplierInformation.EmergencyContact".into(),
            severity: "Critical".into(),
            requirement: "K-GHS (산업안전보건법 별표 18) requires an emergency contact \
                          phone number (국가 응급 대응: 1588-9119 or equivalent)."
                .into(),
            recommendation: "Add EmergencyContact with Phone set to the Korean emergency \
                             response line (1588-9119) or supplier emergency number."
                .into(),
        });
    }

    // ── Section 15: K-REACH / KOSHA reference ───────────────────────────────
    if !regulatory_mentions_keyword(sds, &["K-REACH", "화학물질관리법", "산업안전보건법", "KOSHA"]) {
        gaps.push(ComplianceGap {
            section: 15,
            field: "RegulatoryInformation.OtherLegislation".into(),
            severity: "Major".into(),
            requirement: "K-GHS requires applicable Korean laws (산업안전보건법, \
                          화학물질관리법 / K-REACH) to be listed in Section 15."
                .into(),
            recommendation: "Add OtherLegislation entries referencing K-REACH registration \
                             status and 산업안전보건법 classification."
                .into(),
        });
    }
}

fn check_taiwan(sds: &SdsRoot, gaps: &mut Vec<ComplianceGap>) {
    // ── Section 1: emergency contact ────────────────────────────────────────
    if !has_emergency_contact(sds) {
        gaps.push(ComplianceGap {
            section: 1,
            field: "Identification.SupplierInformation.EmergencyContact".into(),
            severity: "Major".into(),
            requirement: "CNS 15030 requires emergency contact information including \
                          reference to the National Fire Agency hotline."
                .into(),
            recommendation: "Add EmergencyContact referencing 行政院環境保護署 (EPA) or \
                             消防署 毒化災防救諮詢中心 (0800-073-001)."
                .into(),
        });
    }

    // ── Section 15: Toxic Chemical Substances Act reference ──────────────────
    if !regulatory_mentions_keyword(sds, &["毒性化學物質", "毒性及關注化學物質", "職業安全衛生"]) {
        gaps.push(ComplianceGap {
            section: 15,
            field: "RegulatoryInformation.OtherLegislation".into(),
            severity: "Advisory".into(),
            requirement: "CNS 15030 §15 should reference applicable Taiwanese law, \
                          including 毒性及關注化學物質管理法 for listed substances."
                .into(),
            recommendation: "Add OtherLegislation entry for 毒性及關注化學物質管理法 \
                             if the substance is on the Taiwan EPA's toxic chemicals list."
                .into(),
        });
    }
}

fn check_japan(sds: &SdsRoot, gaps: &mut Vec<ComplianceGap>) {
    // Japan is the native format — universal validate() covers most rules.
    // Only add gaps that are Japan-specific and not already in the universal validator.

    // ── Section 15: PRTR / Industrial Safety Law reference ──────────────────
    if sds.regulatory_information.is_none() {
        gaps.push(ComplianceGap {
            section: 15,
            field: "RegulatoryInformation".into(),
            severity: "Advisory".into(),
            requirement: "JIS Z 7253 §15 should reference applicable Japanese laws \
                          (労働安全衛生法, PRTR法, 毒物劇物取締法, etc.)."
                .into(),
            recommendation: "Populate RegulatoryInformation.ISHA or RegulatoryInformation.\
                             PRTRLaw with the substance's Japanese regulatory status."
                .into(),
        });
    }
}

// ---------------------------------------------------------------------------
// Field-presence helpers (shared across country checks)
// ---------------------------------------------------------------------------

fn has_emergency_contact(sds: &SdsRoot) -> bool {
    let id = match sds.identification.as_ref() {
        Some(v) => v,
        None => return false,
    };
    // SupplierInformation
    if let Some(si) = &id.supplier_information {
        if si.emergency_contact.as_ref().map_or(false, |cs| {
            cs.iter().any(|c| c.phone.as_ref().map_or(false, |p| !p.is_empty()))
        }) {
            return true;
        }
    }
    // DomesticManufacturerInformation
    if let Some(dmi) = &id.domestic_manufacturer_information {
        if dmi.emergency_contact.as_ref().map_or(false, |cs| {
            cs.iter().any(|c| c.phone.as_ref().map_or(false, |p| !p.is_empty()))
        }) {
            return true;
        }
    }
    false
}

fn has_acute_toxicity_data(sds: &SdsRoot) -> bool {
    sds.toxicological_information
        .as_ref()
        .map_or(false, |vec| {
            vec.iter().any(|ti| {
                ti.acute_toxicity.as_ref().map_or(false, |at| {
                    at.exposure_route.as_ref().map_or(false, |routes| !routes.is_empty())
                })
            })
        })
}

fn has_exposure_limit_reference(sds: &SdsRoot, keywords: &[&str]) -> bool {
    let ec = match sds.exposure_control_personal_protection.as_ref() {
        Some(v) => v,
        None => return false,
    };
    let Ok(json) = serde_json::to_string(ec) else { return false };
    keywords.iter().any(|kw| json.contains(kw))
}

fn regulatory_mentions_keyword(sds: &SdsRoot, keywords: &[&str]) -> bool {
    let ri = match sds.regulatory_information.as_ref() {
        Some(v) => v,
        None => return false,
    };
    let Ok(json) = serde_json::to_string(ri) else { return false };
    keywords.iter().any(|kw| json.contains(kw))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::SdsRoot;

    #[test]
    fn china_empty_sds_has_critical_gaps() {
        let sds = SdsRoot::default();
        let report = generate_compliance_diff(&sds, SourceCountry::China);
        assert_eq!(report.target_country, "China");
        assert!(report.gap_count >= 2, "should have at least emergency contact + regulatory gap");
        let critical: Vec<_> = report.gaps.iter().filter(|g| g.severity == "Critical").collect();
        assert!(!critical.is_empty(), "at least one Critical gap expected for empty China SDS");
    }

    #[test]
    fn japan_full_sds_no_regulatory_gap() {
        use crate::schema::*;
        let mut sds = SdsRoot::default();
        sds.regulatory_information = Some(RegulatoryInformation::default());
        let report = generate_compliance_diff(&sds, SourceCountry::Japan);
        assert_eq!(report.gap_count, 0, "Japan with RegulatoryInformation should have 0 gaps");
    }

    #[test]
    fn korea_empty_sds_has_emergency_gap() {
        let sds = SdsRoot::default();
        let report = generate_compliance_diff(&sds, SourceCountry::Korea);
        assert!(
            report.gaps.iter().any(|g| g.section == 1),
            "Korea should flag missing emergency contact in section 1"
        );
    }
}
