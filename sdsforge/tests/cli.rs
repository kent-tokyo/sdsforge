//! Subprocess-level CLI tests for the `sdsforge` binary.
//!
//! Covers the canonical `render` command and its deprecated
//! `to-docx`/`to-html`/`to-pdf` aliases against the compiled binary, so exit
//! codes, stdout, and stderr all reflect exactly what a user sees.

use std::process::{Command, Output};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_sdsforge")
}

fn fixture_json() -> &'static str {
    concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../examples/mhlw_allyl_chloride/expected.json"
    )
}

fn run(args: &[&str]) -> Output {
    Command::new(bin())
        .args(args)
        .output()
        .expect("failed to run sdsforge binary")
}

fn out_file(suffix: &str) -> tempfile::TempPath {
    tempfile::Builder::new()
        .suffix(suffix)
        .tempfile()
        .unwrap()
        .into_temp_path()
}

#[test]
fn render_to_docx_succeeds() {
    let out = out_file(".docx");
    let output = run(&[
        "render",
        "--input",
        fixture_json(),
        "--to",
        "docx",
        "--output",
        out.to_str().unwrap(),
    ]);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(out.metadata().unwrap().len() > 0);
}

#[test]
fn render_to_html_succeeds() {
    let out = out_file(".html");
    let output = run(&[
        "render",
        "--input",
        fixture_json(),
        "--to",
        "html",
        "--output",
        out.to_str().unwrap(),
    ]);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(out.metadata().unwrap().len() > 0);
}

/// PDF rendering depends on a system CJK font being installed (see
/// `sdsforge_core::converter::pdf::load_cjk_font`) — not on LibreOffice,
/// which the current implementation doesn't use. CI environments without a
/// CJK font are expected to fail with that specific message rather than
/// succeed or panic.
#[test]
fn render_to_pdf_succeeds_or_reports_missing_font() {
    let out = out_file(".pdf");
    let output = run(&[
        "render",
        "--input",
        fixture_json(),
        "--to",
        "pdf",
        "--output",
        out.to_str().unwrap(),
    ]);
    if output.status.success() {
        assert!(out.metadata().unwrap().len() > 0);
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("CJK font"),
            "unexpected pdf failure: {stderr}"
        );
    }
}

/// HTML is a pure function of the input JSON (no embedded timestamps, unlike
/// docx/pdf), so the canonical command and the deprecated alias must produce
/// byte-identical output — proving they share one implementation.
#[test]
fn render_to_html_matches_legacy_to_html_byte_for_byte() {
    let canonical = out_file(".html");
    let legacy = out_file(".html");

    let canonical_out = run(&[
        "render",
        "--input",
        fixture_json(),
        "--to",
        "html",
        "--output",
        canonical.to_str().unwrap(),
    ]);
    assert!(canonical_out.status.success());

    let legacy_out = run(&[
        "to-html",
        "--input",
        fixture_json(),
        "--output",
        legacy.to_str().unwrap(),
    ]);
    assert!(legacy_out.status.success());

    assert_eq!(
        std::fs::read(&canonical).unwrap(),
        std::fs::read(&legacy).unwrap()
    );
}

#[test]
fn legacy_to_docx_prints_deprecation_warning_to_stderr_only() {
    let out = out_file(".docx");
    let output = run(&[
        "to-docx",
        "--input",
        fixture_json(),
        "--output",
        out.to_str().unwrap(),
    ]);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr
        .contains("warning: `sdsforge to-docx` is deprecated; use `sdsforge render --to docx`"));

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("warning"));
}

#[test]
fn legacy_to_html_prints_deprecation_warning_to_stderr_only() {
    let out = out_file(".html");
    let output = run(&[
        "to-html",
        "--input",
        fixture_json(),
        "--output",
        out.to_str().unwrap(),
    ]);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr
        .contains("warning: `sdsforge to-html` is deprecated; use `sdsforge render --to html`"));

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("warning"));
}

#[test]
fn legacy_to_pdf_prints_deprecation_warning_to_stderr_only() {
    let out = out_file(".pdf");
    let output = run(&[
        "to-pdf",
        "--input",
        fixture_json(),
        "--output",
        out.to_str().unwrap(),
    ]);

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("warning: `sdsforge to-pdf` is deprecated; use `sdsforge render --to pdf`")
    );
    assert!(
        output.status.success() || stderr.contains("CJK font"),
        "unexpected pdf failure: {stderr}"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("warning"));
}

#[test]
fn invalid_to_value_fails_cleanly() {
    let out = out_file(".docx");
    let output = run(&[
        "render",
        "--input",
        fixture_json(),
        "--to",
        "bogus",
        "--output",
        out.to_str().unwrap(),
    ]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("possible values"));
}

#[test]
fn render_missing_required_output_fails_cleanly() {
    let output = run(&["render", "--input", fixture_json(), "--to", "docx"]);
    assert!(!output.status.success());
}
