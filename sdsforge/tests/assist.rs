//! CLI-level tests for `sdsforge assist` (Section 4 / first-aid measures,
//! v1). The mocked-backend test points `--base-url` at a local wiremock
//! server standing in for an OpenAI-compatible API -- no live network
//! access, no real API key, consistent with the rest of this test suite.

use std::path::Path;
use std::process::{Command, Output};

use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_sdsforge")
}

fn run(args: &[&str]) -> Output {
    Command::new(bin())
        .args(args)
        .output()
        .expect("failed to run sdsforge binary")
}

#[test]
fn assist_help_documents_the_command() {
    let output = run(&["assist", "--help"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--source"));
    assert!(stdout.contains("--source-kind"));
    assert!(stdout.contains("--section"));
    assert!(stdout.contains("--output"));
}

#[test]
fn assist_rejects_section_other_than_4_before_touching_network_or_disk() {
    // No API key and a nonexistent --source: the --section check must run
    // (and fail) before either would ever be needed.
    let dir = tempfile::tempdir().unwrap();
    let output = run(&[
        "assist",
        "--source", "does-not-exist.pdf",
        "--source-kind", "supplier-sds",
        "--section", "5",
        "--output", dir.path().join("out.json").to_str().unwrap(),
    ]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("only supports --section 4"), "stderr: {stderr}");
    assert!(std::fs::read_dir(dir.path()).unwrap().next().is_none(), "no file should have been written");
}

/// A short, fictional supplier SDS Section 4 excerpt -- not a real product.
const FIXTURE_SOURCE_TEXT: &str = "\
SAFETY DATA SHEET (fictional, for testing only)
Product: GoldenFix Industrial Degreaser

SECTION 4: FIRST-AID MEASURES
Inhalation: Move person to fresh air. If breathing is difficult, give oxygen.
";

fn openai_compat_body_for(candidates_json_array: &str) -> serde_json::Value {
    serde_json::json!({
        "choices": [
            { "message": { "content": candidates_json_array } }
        ]
    })
}

#[tokio::test]
async fn assist_writes_only_the_requested_output_file() {
    let mock_server = MockServer::start().await;
    let candidates = serde_json::json!([{
        "path": "FirstAidMeasures.ExposureRoute.FirstAidInhalation.FullText",
        "proposed_value": "Move person to fresh air. If breathing is difficult, give oxygen.",
        "source_page": 1,
        "source_excerpt": "Move person to fresh air. If breathing is difficult, give oxygen.",
        "rationale": "quoted from the Inhalation line"
    }])
    .to_string();
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(openai_compat_body_for(&candidates)))
        .mount(&mock_server)
        .await;

    let dir = tempfile::tempdir().unwrap();
    let source_path = dir.path().join("supplier-sds.txt");
    std::fs::write(&source_path, FIXTURE_SOURCE_TEXT).unwrap();
    let output_path = dir.path().join("assist_proposals.json");

    let output = run(&[
        "assist",
        "--source", source_path.to_str().unwrap(),
        "--source-kind", "supplier-sds",
        "--section", "4",
        "--output", output_path.to_str().unwrap(),
        "--provider", "openai",
        "--base-url", &mock_server.uri(),
        "--api-key", "test-key-not-real",
        "--model", "test-model",
    ]);
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    assert!(output_path.exists());
    let run_json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&output_path).unwrap()).unwrap();
    assert_eq!(run_json["proposals"].as_array().unwrap().len(), 1);
    assert_eq!(run_json["source_evidence_level"], "supplier_sds");
    assert_eq!(run_json["extraction_method"], "llm_extraction");

    // Nothing else may appear in the output directory -- no official_sds.json,
    // generation_report.json, review_report.md, or authoring-input file.
    let entries: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .filter(|name| Path::new(name) != source_path.file_name().unwrap())
        .collect();
    assert_eq!(entries, vec!["assist_proposals.json".to_string()]);
}
