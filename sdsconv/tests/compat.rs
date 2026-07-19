//! Confirms the deprecated `sdsconv` binary forwards into the same CLI
//! implementation as `sdsforge` instead of just printing a message and
//! exiting 1.

use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_sdsconv")
}

fn fixture_json() -> &'static str {
    concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../examples/mhlw_allyl_chloride/expected.json"
    )
}

#[test]
fn sdsconv_to_docx_forwards_and_succeeds() {
    let out = tempfile::Builder::new()
        .suffix(".docx")
        .tempfile()
        .unwrap()
        .into_temp_path();
    let output = Command::new(bin())
        .args([
            "to-docx",
            "--input",
            fixture_json(),
            "--output",
            out.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run sdsconv binary");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(out.metadata().unwrap().len() > 0);

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("warning: the `sdsconv` command has been renamed to `sdsforge`"));
    assert!(stderr
        .contains("warning: `sdsforge to-docx` is deprecated; use `sdsforge render --to docx`"));

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("warning"));
}

fn example_generate_yaml() -> &'static str {
    concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../examples/generate/example_cleaner.yaml"
    )
}

/// `sdsforge generate` is new (not just an existing command), so this
/// specifically proves it wasn't missed when the compat binary's forwarding
/// was set up -- there is no second `generate` implementation in this crate.
#[test]
fn sdsconv_generate_forwards_and_succeeds() {
    let out_dir = tempfile::tempdir().unwrap();
    let output = Command::new(bin())
        .args([
            "generate",
            "--input",
            example_generate_yaml(),
            "--output-dir",
            out_dir.path().to_str().unwrap(),
        ])
        .output()
        .expect("failed to run sdsconv binary");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(out_dir.path().join("official_sds.json").exists());
    assert!(out_dir.path().join("generation_report.json").exists());
    assert!(out_dir.path().join("review_report.md").exists());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("warning: the `sdsconv` command has been renamed to `sdsforge`"));
    assert!(stderr.contains("Generated SDS draft"));

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.is_empty());
}

/// The old binary shares clap's parser via `sdsforge::run_cli_from` rather
/// than reimplementing it, so an invalid `--to` value must fail the same way
/// through `sdsconv` as it does through `sdsforge` directly.
#[test]
fn invalid_to_value_fails_cleanly_through_old_binary_too() {
    let out = tempfile::Builder::new()
        .suffix(".docx")
        .tempfile()
        .unwrap()
        .into_temp_path();
    let output = Command::new(bin())
        .args([
            "render",
            "--input",
            fixture_json(),
            "--to",
            "bogus",
            "--output",
            out.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run sdsconv binary");

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("possible values"));
}
