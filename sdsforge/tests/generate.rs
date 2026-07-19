//! Subprocess-level CLI tests for `sdsforge generate`.
//!
//! Covers the offline (default) path only -- no test here performs live
//! network access. `--enrich` behavior against real PubChem is exercised
//! manually, not in automated tests, since it would make CI depend on
//! external network availability.

use std::path::Path;
use std::process::{Command, Output};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_sdsforge")
}

fn example_yaml() -> &'static str {
    concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../examples/generate/example_cleaner.yaml"
    )
}

fn run(args: &[&str]) -> Output {
    Command::new(bin())
        .args(args)
        .output()
        .expect("failed to run sdsforge binary")
}

fn three_artifacts_exist(dir: &Path) -> bool {
    dir.join("official_sds.json").exists()
        && dir.join("generation_report.json").exists()
        && dir.join("review_report.md").exists()
}

#[test]
fn generate_help_documents_the_command() {
    let output = run(&["generate", "--help"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--input"));
    assert!(stdout.contains("--output-dir"));
    assert!(stdout.contains("--enrich"));
    assert!(stdout.contains("--strict"));
    assert!(stdout.contains("--force"));
}

#[test]
fn json_input_generates_all_three_files() {
    let dir = tempfile::tempdir().unwrap();
    // Convert the committed YAML fixture to JSON so this test covers the
    // JSON input path with the exact same product data as the YAML test.
    let yaml_text = std::fs::read_to_string(example_yaml()).unwrap();
    let value: serde_json::Value = serde_norway::from_str(&yaml_text).unwrap();
    let json_path = dir.path().join("input.json");
    std::fs::write(&json_path, serde_json::to_string_pretty(&value).unwrap()).unwrap();

    let out_dir = dir.path().join("out");
    let output = run(&[
        "generate",
        "--input",
        json_path.to_str().unwrap(),
        "--output-dir",
        out_dir.to_str().unwrap(),
    ]);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(three_artifacts_exist(&out_dir));
}

#[test]
fn yaml_input_generates_all_three_files() {
    let dir = tempfile::tempdir().unwrap();
    let out_dir = dir.path().join("out");
    let output = run(&[
        "generate",
        "--input",
        example_yaml(),
        "--output-dir",
        out_dir.to_str().unwrap(),
    ]);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(three_artifacts_exist(&out_dir));
}

#[test]
fn unknown_input_extension_fails_cleanly() {
    let dir = tempfile::tempdir().unwrap();
    let bad_input = dir.path().join("input.txt");
    std::fs::copy(example_yaml(), &bad_input).unwrap();
    let out_dir = dir.path().join("out");

    let output = run(&[
        "generate",
        "--input",
        bad_input.to_str().unwrap(),
        "--output-dir",
        out_dir.to_str().unwrap(),
    ]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unsupported input extension"));
    assert!(!out_dir.exists());
}

#[test]
fn malformed_json_writes_no_final_artifacts() {
    let dir = tempfile::tempdir().unwrap();
    let bad_input = dir.path().join("input.json");
    std::fs::write(&bad_input, "{ not valid json").unwrap();
    let out_dir = dir.path().join("out");

    let output = run(&[
        "generate",
        "--input",
        bad_input.to_str().unwrap(),
        "--output-dir",
        out_dir.to_str().unwrap(),
    ]);
    assert!(!output.status.success());
    assert!(!three_artifacts_exist(&out_dir));
}

#[test]
fn malformed_yaml_writes_no_final_artifacts() {
    let dir = tempfile::tempdir().unwrap();
    let bad_input = dir.path().join("input.yaml");
    std::fs::write(&bad_input, "trade_name: [unterminated").unwrap();
    let out_dir = dir.path().join("out");

    let output = run(&[
        "generate",
        "--input",
        bad_input.to_str().unwrap(),
        "--output-dir",
        out_dir.to_str().unwrap(),
    ]);
    assert!(!output.status.success());
    assert!(!three_artifacts_exist(&out_dir));
}

#[test]
fn existing_target_files_cause_failure_without_force() {
    let dir = tempfile::tempdir().unwrap();
    let out_dir = dir.path().join("out");

    let first = run(&[
        "generate",
        "--input",
        example_yaml(),
        "--output-dir",
        out_dir.to_str().unwrap(),
    ]);
    assert!(first.status.success());

    let second = run(&[
        "generate",
        "--input",
        example_yaml(),
        "--output-dir",
        out_dir.to_str().unwrap(),
    ]);
    assert!(!second.status.success());
    assert!(String::from_utf8_lossy(&second.stderr).contains("already exists"));
}

#[test]
fn force_replaces_all_three_outputs() {
    let dir = tempfile::tempdir().unwrap();
    let out_dir = dir.path().join("out");

    let first = run(&[
        "generate",
        "--input",
        example_yaml(),
        "--output-dir",
        out_dir.to_str().unwrap(),
    ]);
    assert!(first.status.success());

    let second = run(&[
        "generate",
        "--input",
        example_yaml(),
        "--output-dir",
        out_dir.to_str().unwrap(),
        "--force",
    ]);
    assert!(
        second.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert!(three_artifacts_exist(&out_dir));
}

#[test]
fn stdout_contains_no_artifact_body_and_status_goes_to_stderr() {
    let dir = tempfile::tempdir().unwrap();
    let out_dir = dir.path().join("out");
    let output = run(&[
        "generate",
        "--input",
        example_yaml(),
        "--output-dir",
        out_dir.to_str().unwrap(),
    ]);
    assert!(output.status.success());
    assert!(
        output.stdout.is_empty(),
        "stdout was not empty: {:?}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Generated SDS draft"));
    assert!(stderr.contains("Release status"));
}

#[test]
fn strict_mode_writes_blocked_artifacts_and_returns_nonzero() {
    let dir = tempfile::tempdir().unwrap();
    // Invalid CAS check digit -> GEN-CAS-CHECKDIGIT (HIGH) -> Blocked.
    let input = dir.path().join("input.json");
    std::fs::write(
        &input,
        r#"{
            "trade_name": "Bad CAS Test",
            "other_names": [],
            "supplier": {"company_name": "Acme", "address": null, "phone": null, "email": null},
            "components": [
                {"cas_number": "7732-18-6", "name": "Water",
                 "concentration": {"exact": 100.0, "lower": null, "upper": null, "unit": "%"}}
            ],
            "measured_properties": {"flash_point": [], "boiling_point": [], "vapor_pressure": [],
                "explosive_limits": [], "self_reactivity": [], "oxidizing_properties": [],
                "metal_corrosivity": []},
            "evidence": []
        }"#,
    )
    .unwrap();
    let out_dir = dir.path().join("out");

    let output = run(&[
        "generate",
        "--input",
        input.to_str().unwrap(),
        "--output-dir",
        out_dir.to_str().unwrap(),
        "--strict",
    ]);
    assert!(!output.status.success());
    assert!(three_artifacts_exist(&out_dir));
    let report: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(out_dir.join("generation_report.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(report["release_status"], "blocked");
}

#[test]
fn normal_mode_writes_blocked_artifacts_and_exits_successfully() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("input.json");
    std::fs::write(
        &input,
        r#"{
            "trade_name": "Bad CAS Test",
            "other_names": [],
            "supplier": {"company_name": "Acme", "address": null, "phone": null, "email": null},
            "components": [
                {"cas_number": "7732-18-6", "name": "Water",
                 "concentration": {"exact": 100.0, "lower": null, "upper": null, "unit": "%"}}
            ],
            "measured_properties": {"flash_point": [], "boiling_point": [], "vapor_pressure": [],
                "explosive_limits": [], "self_reactivity": [], "oxidizing_properties": [],
                "metal_corrosivity": []},
            "evidence": []
        }"#,
    )
    .unwrap();
    let out_dir = dir.path().join("out");

    let output = run(&[
        "generate",
        "--input",
        input.to_str().unwrap(),
        "--output-dir",
        out_dir.to_str().unwrap(),
    ]);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(three_artifacts_exist(&out_dir));
    assert!(String::from_utf8_lossy(&output.stderr).contains("BLOCKED"));
}

/// The committed example has no invalid/duplicate CAS and one unresolved
/// (but nonblocking) property set -- ReviewRequired, not Blocked.
#[test]
fn review_required_without_blocking_fields_exits_successfully() {
    let dir = tempfile::tempdir().unwrap();
    let out_dir = dir.path().join("out");
    let output = run(&[
        "generate",
        "--input",
        example_yaml(),
        "--output-dir",
        out_dir.to_str().unwrap(),
    ]);
    assert!(output.status.success());
    let report: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(out_dir.join("generation_report.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(report["release_status"], "review_required");
}

#[test]
fn repeated_offline_runs_produce_byte_equivalent_files() {
    let dir = tempfile::tempdir().unwrap();
    let out_a = dir.path().join("a");
    let out_b = dir.path().join("b");

    for out_dir in [&out_a, &out_b] {
        let output = run(&[
            "generate",
            "--input",
            example_yaml(),
            "--output-dir",
            out_dir.to_str().unwrap(),
        ]);
        assert!(output.status.success());
    }

    for name in [
        "official_sds.json",
        "generation_report.json",
        "review_report.md",
    ] {
        assert_eq!(
            std::fs::read(out_a.join(name)).unwrap(),
            std::fs::read(out_b.join(name)).unwrap(),
            "{name} differs between repeated runs"
        );
    }
}

#[test]
fn missing_boiling_point_remains_unresolved_with_required_evidence() {
    let dir = tempfile::tempdir().unwrap();
    let out_dir = dir.path().join("out");
    run(&[
        "generate",
        "--input",
        example_yaml(),
        "--output-dir",
        out_dir.to_str().unwrap(),
    ]);
    let report: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(out_dir.join("generation_report.json")).unwrap(),
    )
    .unwrap();
    let unresolved = report["unresolved"].as_array().unwrap();
    assert!(unresolved.iter().any(
        |u| u["path"].as_str().unwrap_or("").contains("BoilingPoint")
            || u["title"]
                .as_str()
                .unwrap_or("")
                .to_lowercase()
                .contains("boiling")
    ));
}

#[test]
fn evidence_backed_flash_point_appears_in_official_json_only_with_metadata_in_report() {
    let dir = tempfile::tempdir().unwrap();
    let out_dir = dir.path().join("out");
    run(&[
        "generate",
        "--input",
        example_yaml(),
        "--output-dir",
        out_dir.to_str().unwrap(),
    ]);

    let official = std::fs::read_to_string(out_dir.join("official_sds.json")).unwrap();
    assert!(official.contains("100"));
    // Method is a real MHLW schema field on FlashPoint, so it legitimately
    // appears here too -- but evidence-tracking metadata (sample/batch IDs,
    // which have no schema field) must never leak into the official SDS,
    // only into the generation report.
    assert!(!official.contains("LOT-2026-0341"));

    let report = std::fs::read_to_string(out_dir.join("generation_report.json")).unwrap();
    assert!(report.contains("LOT-2026-0341"));
    assert!(report.contains("ASTM D93"));
}

#[test]
fn review_report_clearly_says_the_draft_is_unapproved() {
    let dir = tempfile::tempdir().unwrap();
    let out_dir = dir.path().join("out");
    run(&[
        "generate",
        "--input",
        example_yaml(),
        "--output-dir",
        out_dir.to_str().unwrap(),
    ]);
    let review = std::fs::read_to_string(out_dir.join("review_report.md")).unwrap();
    assert!(review.contains("has not been approved"));
}

#[test]
fn official_json_contains_no_report_keys() {
    let dir = tempfile::tempdir().unwrap();
    let out_dir = dir.path().join("out");
    run(&[
        "generate",
        "--input",
        example_yaml(),
        "--output-dir",
        out_dir.to_str().unwrap(),
    ]);
    let official: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out_dir.join("official_sds.json")).unwrap())
            .unwrap();
    let keys: Vec<&str> = official
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    for forbidden in [
        "release_status",
        "findings",
        "unresolved",
        "provenance",
        "evidence_summary",
    ] {
        assert!(!keys.contains(&forbidden));
    }
}

#[test]
fn invalid_profile_value_rejected_by_clap() {
    let dir = tempfile::tempdir().unwrap();
    let out_dir = dir.path().join("out");
    let output = run(&[
        "generate",
        "--input",
        example_yaml(),
        "--output-dir",
        out_dir.to_str().unwrap(),
        "--profile",
        "osha-v1",
    ]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("possible values"));
}

// -- serde(default)/deny_unknown_fields input UX (concise inputs) --

const MINIMAL_YAML: &str = r#"
trade_name: Minimal Product
supplier:
  company_name: Acme
components:
  - concentration:
      exact: 100.0
      unit: "%"
"#;

const MINIMAL_JSON: &str = r#"{
    "trade_name": "Minimal Product",
    "supplier": {"company_name": "Acme"},
    "components": [
        {"concentration": {"exact": 100.0, "unit": "%"}}
    ]
}"#;

/// Every field the concise fixtures above omit, spelled out explicitly.
/// Must produce byte-identical artifacts to the concise form.
const VERBOSE_EQUIVALENT_YAML: &str = r#"
trade_name: Minimal Product
other_names: []
supplier:
  company_name: Acme
  address: null
  phone: null
  email: null
components:
  - cas_number: null
    name: null
    concentration:
      exact: 100.0
      lower: null
      upper: null
      unit: "%"
measured_properties:
  flash_point: []
  boiling_point: []
  vapor_pressure: []
  explosive_limits: []
  self_reactivity: []
  oxidizing_properties: []
  metal_corrosivity: []
evidence: []
"#;

#[test]
fn minimal_yaml_generates_successfully() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("input.yaml");
    std::fs::write(&input, MINIMAL_YAML).unwrap();
    let out_dir = dir.path().join("out");

    let output = run(&[
        "generate",
        "--input",
        input.to_str().unwrap(),
        "--output-dir",
        out_dir.to_str().unwrap(),
    ]);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(three_artifacts_exist(&out_dir));
}

#[test]
fn minimal_json_generates_successfully() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("input.json");
    std::fs::write(&input, MINIMAL_JSON).unwrap();
    let out_dir = dir.path().join("out");

    let output = run(&[
        "generate",
        "--input",
        input.to_str().unwrap(),
        "--output-dir",
        out_dir.to_str().unwrap(),
    ]);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(three_artifacts_exist(&out_dir));
}

#[test]
fn verbose_and_concise_inputs_generate_byte_equivalent_artifacts() {
    let dir = tempfile::tempdir().unwrap();
    let concise_input = dir.path().join("concise.yaml");
    std::fs::write(&concise_input, MINIMAL_YAML).unwrap();
    let verbose_input = dir.path().join("verbose.yaml");
    std::fs::write(&verbose_input, VERBOSE_EQUIVALENT_YAML).unwrap();

    let concise_out = dir.path().join("concise_out");
    let verbose_out = dir.path().join("verbose_out");

    for (input, out_dir) in [
        (&concise_input, &concise_out),
        (&verbose_input, &verbose_out),
    ] {
        let output = run(&[
            "generate",
            "--input",
            input.to_str().unwrap(),
            "--output-dir",
            out_dir.to_str().unwrap(),
        ]);
        assert!(
            output.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    for name in [
        "official_sds.json",
        "generation_report.json",
        "review_report.md",
    ] {
        assert_eq!(
            std::fs::read(concise_out.join(name)).unwrap(),
            std::fs::read(verbose_out.join(name)).unwrap(),
            "{name} differs between concise and verbose input"
        );
    }
}

#[test]
fn example_cleaner_concise_form_matches_fully_expanded_equivalent() {
    // The committed example was simplified to the concise form as part of
    // this change. Re-expand every now-implicit default by hand and prove
    // it still produces byte-identical artifacts to the committed file.
    const FULLY_EXPANDED: &str = r#"
trade_name: "AllClean Multi-Surface Cleaner"
other_names: []
supplier:
  company_name: "Example Chemical Co., Ltd."
  address: "1-1 Example, Chiyoda-ku, Tokyo"
  phone: "03-1234-5678"
  email: "safety@example.com"
components:
  - cas_number: "7732-18-5"
    name: "Water"
    concentration:
      exact: 85.0
      lower: null
      upper: null
      unit: "%"
  - cas_number: "151-21-3"
    name: "Sodium Lauryl Sulfate"
    concentration:
      exact: null
      lower: 5.0
      upper: 15.0
      unit: "%"
measured_properties:
  flash_point:
    - value: 100.0
      unit: "degC"
      method: "Closed Cup (ASTM D93)"
      conditions:
        temperature_c: 20.0
        pressure_kpa: null
        atmosphere: null
      sample_id: "LOT-2026-0341"
      batch_id: null
      evidence_id: "ev-flash-point-1"
  boiling_point: []
  vapor_pressure: []
  explosive_limits: []
  self_reactivity: []
  oxidizing_properties: []
  metal_corrosivity: []
evidence:
  - id: "ev-flash-point-1"
    level: "product_test_report"
    reference: "Internal Lab Report FP-2026-0341"
    issuer: "Example Chemical Co. QA Lab"
    document_date: "2026-05-12"
    applies_to: "finished_product"
"#;
    let dir = tempfile::tempdir().unwrap();
    let expanded_input = dir.path().join("expanded.yaml");
    std::fs::write(&expanded_input, FULLY_EXPANDED).unwrap();

    let concise_out = dir.path().join("concise_out");
    let expanded_out = dir.path().join("expanded_out");

    for (input, out_dir) in [
        (example_yaml().to_string(), &concise_out),
        (expanded_input.to_str().unwrap().to_string(), &expanded_out),
    ] {
        let output = run(&[
            "generate",
            "--input",
            &input,
            "--output-dir",
            out_dir.to_str().unwrap(),
        ]);
        assert!(
            output.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    for name in [
        "official_sds.json",
        "generation_report.json",
        "review_report.md",
    ] {
        assert_eq!(
            std::fs::read(concise_out.join(name)).unwrap(),
            std::fs::read(expanded_out.join(name)).unwrap(),
            "{name} differs between the committed example and its fully-expanded equivalent"
        );
    }
}

#[test]
fn unknown_top_level_field_fails_with_actionable_error_and_writes_no_artifacts() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("input.yaml");
    std::fs::write(
        &input,
        r#"
trade_name: Typo Product
supplier:
  company_name: Acme
components:
  - concentration:
      exact: 100.0
      unit: "%"
bogus_field: true
"#,
    )
    .unwrap();
    let out_dir = dir.path().join("out");

    let output = run(&[
        "generate",
        "--input",
        input.to_str().unwrap(),
        "--output-dir",
        out_dir.to_str().unwrap(),
    ]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("bogus_field"));
    assert!(!three_artifacts_exist(&out_dir));
}

#[test]
fn misspelled_concentration_field_fails_with_actionable_error_and_writes_no_artifacts() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("input.yaml");
    std::fs::write(
        &input,
        r#"
trade_name: Typo Product
supplier:
  company_name: Acme
components:
  - cas_number: "7732-18-5"
    name: Water
    concentation:
      exact: 100.0
      unit: "%"
"#,
    )
    .unwrap();
    let out_dir = dir.path().join("out");

    let output = run(&[
        "generate",
        "--input",
        input.to_str().unwrap(),
        "--output-dir",
        out_dir.to_str().unwrap(),
    ]);
    assert!(!output.status.success());
    assert!(!three_artifacts_exist(&out_dir));
    let stderr = String::from_utf8_lossy(&output.stderr);
    // The typo ("concentation") is an unknown field, and the real required
    // key ("concentration") is then missing -- either name being present in
    // the error is actionable; assert on the fields, not the exact wording.
    assert!(stderr.contains("concentation") || stderr.contains("concentration"));
}

#[test]
fn missing_trade_name_fails_via_cli() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("input.yaml");
    std::fs::write(
        &input,
        r#"
supplier:
  company_name: Acme
components:
  - concentration:
      exact: 100.0
      unit: "%"
"#,
    )
    .unwrap();
    let out_dir = dir.path().join("out");

    let output = run(&[
        "generate",
        "--input",
        input.to_str().unwrap(),
        "--output-dir",
        out_dir.to_str().unwrap(),
    ]);
    assert!(!output.status.success());
    assert!(!three_artifacts_exist(&out_dir));
    assert!(String::from_utf8_lossy(&output.stderr).contains("trade_name"));
}
