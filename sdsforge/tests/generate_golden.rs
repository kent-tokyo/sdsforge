//! Golden acceptance tests for `sdsforge generate`.
//!
//! Field-level generation logic already has extensive coverage in
//! `sdsforge_core/src/generation/{draft,result}.rs` and CLI-integration
//! coverage in `generate.rs`. This file adds whole-document regression
//! tests: a complete generated `official_sds.json` compared against a
//! fixed, human-reviewed expected file for representative products, plus
//! MHLW format-conformance checking (`sdsforge_core::validate_typed`).
//!
//! Two independent validation layers are checked per scenario, since they
//! catch different things and use different rule catalogs:
//! - `sdsforge_core::validate_typed` — general MHLW structural/GHS/CAS
//!   conformance rules (the same mechanism `--strict-mhlw` uses). Every
//!   `generate` output shows 14 `WARN STRUCTURAL "section not
//!   extracted"` findings for Sections 2/4-16 (13 when
//!   `PhysicalChemicalProperties` is populated) -- expected and inherent
//!   to `generate`'s Section-1/3(/9)-only scope, not a defect, and never
//!   asserted on directly here.
//! - `generate`'s own findings in `generation_report.json` (rule IDs like
//!   `GEN-CAS-CHECKDIGIT`) -- these are what actually drive
//!   `release_status` (`Blocked`/`ReviewRequired`).
//!
//! Fully offline throughout -- no `--enrich`, no network dependency.
//! Fixtures for the salt/multi-fragment scenario (which needs chematic
//! normalization, only reachable via `--enrich` in production) live in
//! `sdsforge_core/tests/generate_golden.rs` instead, exercised via direct
//! library calls with a hand-built resolved candidate -- no HTTP mocking
//! seam needed.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use sdsforge_core::{validate_typed, SdsRoot};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_sdsforge")
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/generate_golden")
        .join(name)
}

fn run(args: &[&str]) -> Output {
    Command::new(bin())
        .args(args)
        .output()
        .expect("failed to run sdsforge binary")
}

/// `"review_required"` -> `"REVIEW REQUIRED"`, matching
/// `describe_release_status`'s wording in `review_report.md`.
fn shout_case(snake_case: &str) -> String {
    snake_case.replace('_', " ").to_uppercase()
}

struct GoldenRun {
    official: serde_json::Value,
    report: serde_json::Value,
    review: String,
}

fn generate_golden(input_name: &str, out_dir: &Path) -> (Output, Option<GoldenRun>) {
    let output = run(&[
        "generate",
        "--input",
        fixture(input_name).to_str().unwrap(),
        "--output-dir",
        out_dir.to_str().unwrap(),
    ]);
    if !out_dir.join("official_sds.json").exists() {
        return (output, None);
    }
    let official: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out_dir.join("official_sds.json")).unwrap())
            .unwrap();
    let report: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(out_dir.join("generation_report.json")).unwrap(),
    )
    .unwrap();
    let review = std::fs::read_to_string(out_dir.join("review_report.md")).unwrap();
    (
        output,
        Some(GoldenRun {
            official,
            report,
            review,
        }),
    )
}

fn assert_official_matches_golden(actual: &serde_json::Value, golden_file: &str) {
    let expected: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(fixture(golden_file)).unwrap()).unwrap();
    assert_eq!(
        actual, &expected,
        "official_sds.json does not match the golden fixture"
    );
}

/// `validate_typed` CRIT must always be zero; its own HIGH set (a
/// different rule catalog than `generate`'s findings, see module docs)
/// must equal `expected_high`.
fn assert_conformance(official: &serde_json::Value, expected_high: &BTreeSet<&str>) {
    let sds: SdsRoot = serde_json::from_value(official.clone()).unwrap();
    let findings = validate_typed(&sds);
    assert!(
        findings.iter().all(|f| f.level != "CRIT"),
        "unexpected CRIT findings: {findings:#?}"
    );
    let actual_high: BTreeSet<&str> = findings
        .iter()
        .filter(|f| f.level == "HIGH")
        .map(|f| f.rule.as_str())
        .collect();
    assert_eq!(
        &actual_high, expected_high,
        "validate_typed HIGH set mismatch"
    );
}

/// `generate`'s own findings (in `generation_report.json`, not
/// `validate_typed`'s) are what actually compute `release_status`. This
/// is the check that proves a blocking condition is never silently
/// swallowed.
fn assert_generation_high_findings(report: &serde_json::Value, expected_high: &BTreeSet<&str>) {
    let actual_high: BTreeSet<&str> = report["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["level"] == "HIGH")
        .map(|f| f["rule"].as_str().unwrap())
        .collect();
    assert_eq!(
        &actual_high, expected_high,
        "generation findings HIGH set mismatch"
    );
}

fn assert_unresolved_paths(report: &serde_json::Value, expected: &BTreeSet<&str>) {
    let actual: BTreeSet<&str> = report["unresolved"]
        .as_array()
        .unwrap()
        .iter()
        .map(|u| u["path"].as_str().unwrap())
        .collect();
    assert_eq!(&actual, expected, "unresolved field-path set mismatch");
}

fn assert_release_status(report: &serde_json::Value, review: &str, expected: &str) {
    assert_eq!(report["release_status"], expected);
    assert!(
        review.contains(&shout_case(expected)),
        "review_report.md does not mention release status {expected:?}"
    );
    let unresolved_count = report["unresolved"].as_array().unwrap().len();
    assert!(
        review.contains(&format!("Unresolved fields: {unresolved_count}")),
        "review_report.md's unresolved count does not match generation_report.json's {unresolved_count}"
    );
}

/// `generate` only ever populates `Identification`, `Composition`, and
/// (when measured-property evidence was supplied) `PhysicalChemicalProperties`
/// -- Section 2 (GHS classification) and every other not-yet-implemented
/// section must never be fabricated as a placeholder.
fn assert_no_fabricated_sections(official: &serde_json::Value, expected_keys: &BTreeSet<&str>) {
    let actual: BTreeSet<&str> = official
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        &actual, expected_keys,
        "unexpected top-level SdsRoot section present"
    );
}

#[test]
fn single_substance_ethanol_matches_golden() {
    let dir = tempfile::tempdir().unwrap();
    let (output, run) = generate_golden("single_substance_ethanol.yaml", &dir.path().join("out"));
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let run = run.unwrap();

    assert_official_matches_golden(
        &run.official,
        "single_substance_ethanol.expected_official_sds.json",
    );
    assert_no_fabricated_sections(
        &run.official,
        &BTreeSet::from(["Composition", "Identification"]),
    );
    assert_conformance(&run.official, &BTreeSet::new());
    assert_generation_high_findings(&run.report, &BTreeSet::new());
    assert_release_status(&run.report, &run.review, "review_required");
    assert_unresolved_paths(
        &run.report,
        &BTreeSet::from([
            "Composition.CompositionAndConcentration[0].SubstanceIdentifiers.SubstanceIdentity.CASno",
            "HazardIdentification.Classification.PhysicochemicalEffect.CorrosiveToMetals",
            "PhysicalChemicalProperties.ExplosiveLimits",
            "PhysicalChemicalProperties.FlashPoint",
            "PhysicalChemicalProperties.InitialBoilingPointAndBoilingRange",
            "PhysicalChemicalProperties.OxidizingProperties",
            "PhysicalChemicalProperties.VapourPressure",
            "StabilityReactivity.SelfReactivity",
        ]),
    );
}

#[test]
fn aqueous_mixture_evidence_matches_golden() {
    let dir = tempfile::tempdir().unwrap();
    let (output, run) = generate_golden("aqueous_mixture_evidence.yaml", &dir.path().join("out"));
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let run = run.unwrap();

    assert_official_matches_golden(
        &run.official,
        "aqueous_mixture_evidence.expected_official_sds.json",
    );
    assert_no_fabricated_sections(
        &run.official,
        &BTreeSet::from([
            "Composition",
            "Identification",
            "PhysicalChemicalProperties",
        ]),
    );
    assert_conformance(&run.official, &BTreeSet::new());
    assert_generation_high_findings(&run.report, &BTreeSet::new());
    assert_release_status(&run.report, &run.review, "review_required");
    assert_unresolved_paths(
        &run.report,
        &BTreeSet::from([
            "Composition.CompositionAndConcentration[0].SubstanceIdentifiers.SubstanceIdentity.CASno",
            "Composition.CompositionAndConcentration[1].SubstanceIdentifiers.SubstanceIdentity.CASno",
            "HazardIdentification.Classification.PhysicochemicalEffect.CorrosiveToMetals",
            "PhysicalChemicalProperties.ExplosiveLimits",
            "PhysicalChemicalProperties.InitialBoilingPointAndBoilingRange",
            "PhysicalChemicalProperties.OxidizingProperties",
            "PhysicalChemicalProperties.VapourPressure",
            "StabilityReactivity.SelfReactivity",
        ]),
    );

    // Evidence-tracking metadata (sample/batch IDs) must reach the report
    // but never leak into the official SDS -- only the schema field
    // (Method, via the input's flash-point `method`) legitimately appears
    // in both.
    let official_text = run.official.to_string();
    assert!(!official_text.contains("GOLDEN-LOT-0002"));
    assert!(!official_text.contains("GOLDEN-BATCH-0002"));
    let report_text = run.report.to_string();
    assert!(report_text.contains("GOLDEN-LOT-0002"));
    assert!(report_text.contains("GOLDEN-BATCH-0002"));

    // Provenance spot-check: the evidence-backed flash point must be
    // recorded at the evidence level the input actually supplied, not
    // silently downgraded (or upgraded) in transit.
    let flash_point_provenance = run.report["provenance"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["path"] == "PhysicalChemicalProperties.FlashPoint")
        .expect("FlashPoint provenance entry missing from generation_report.json");
    assert_eq!(flash_point_provenance["source_type"], "product_test_report");
    assert_eq!(flash_point_provenance["confidence"], "high");
}

#[test]
fn blocked_invalid_cas_matches_golden() {
    let dir = tempfile::tempdir().unwrap();
    let (output, run) = generate_golden("blocked_invalid_cas.yaml", &dir.path().join("out"));
    // Non-strict mode still exits 0 and writes artifacts even when Blocked.
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let run = run.unwrap();

    assert_official_matches_golden(
        &run.official,
        "blocked_invalid_cas.expected_official_sds.json",
    );
    assert_no_fabricated_sections(
        &run.official,
        &BTreeSet::from(["Composition", "Identification"]),
    );
    // The invalid CAS is preserved as-supplied, never silently corrected
    // or dropped.
    assert_eq!(
        run.official["Composition"]["CompositionAndConcentration"][0]["SubstanceIdentifiers"]
            ["SubstanceIdentity"]["CASno"]["FullText"][0],
        "7732-18-6"
    );
    // validate_typed independently flags the same check-digit problem,
    // but only at WARN (its own rule catalog, not blocking on its own).
    assert_conformance(&run.official, &BTreeSet::new());
    // generate's own findings are what actually block release.
    assert_generation_high_findings(&run.report, &BTreeSet::from(["GEN-CAS-CHECKDIGIT"]));
    assert_release_status(&run.report, &run.review, "blocked");
    assert_unresolved_paths(
        &run.report,
        &BTreeSet::from([
            "HazardIdentification.Classification.PhysicochemicalEffect.CorrosiveToMetals",
            "PhysicalChemicalProperties.ExplosiveLimits",
            "PhysicalChemicalProperties.FlashPoint",
            "PhysicalChemicalProperties.InitialBoilingPointAndBoilingRange",
            "PhysicalChemicalProperties.OxidizingProperties",
            "PhysicalChemicalProperties.VapourPressure",
            "StabilityReactivity.SelfReactivity",
        ]),
    );
}

#[test]
fn blocked_invalid_cas_strict_mode_exits_nonzero() {
    let dir = tempfile::tempdir().unwrap();
    let out_dir = dir.path().join("out");
    let output = run(&[
        "generate",
        "--input",
        fixture("blocked_invalid_cas.yaml").to_str().unwrap(),
        "--output-dir",
        out_dir.to_str().unwrap(),
        "--strict",
    ]);
    assert!(!output.status.success());
    assert!(out_dir.join("official_sds.json").exists());
    assert!(out_dir.join("generation_report.json").exists());
    assert!(out_dir.join("review_report.md").exists());
}
