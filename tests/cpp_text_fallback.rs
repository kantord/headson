mod common;

#[test]
fn cpp_text_fallback_snapshot() {
    // Use a real C++-like file with indentation so future changes to
    // text fallback (e.g., indent-aware rendering) will reflect in the snapshot.
    let fixture = std::path::Path::new("tests/fixtures/code/sample.cpp");

    let out = common::run_cli(
        &[
            "--no-color", // stabilize output
            "-c",
            "120", // modest char budget to potentially trigger omission markers
            "-f",
            "auto", // unknown ext => text template fallback
            fixture.to_str().unwrap(),
        ],
        None,
    );

    let out = common::normalize_trailing_newline(&out.stdout);
    insta::assert_snapshot!(out);
}

#[test]
fn cpp_text_fallback_snapshot_json() {
    let fixture = std::path::Path::new("tests/fixtures/code/sample.cpp");
    let out = common::run_cli(
        &[
            "--no-color",
            "-c",
            "120",
            // Force text ingest, but render with JSON template for structure visibility
            "-i",
            "text",
            "-f",
            "json",
            fixture.to_str().unwrap(),
        ],
        None,
    );

    let out = common::normalize_trailing_newline(&out.stdout);
    insta::assert_snapshot!(out);
}

#[test]
fn code_format_override_text_template() {
    let fixture = std::path::Path::new("tests/fixtures/code/sample.py");
    let out = common::run_cli(
        &[
            "--no-color",
            "-c",
            "120",
            "-i",
            "text",
            "-f",
            "text",
            fixture.to_str().unwrap(),
        ],
        None,
    );
    let out = out.stdout;
    assert!(
        out.starts_with("def greet"),
        "expected raw text output, got: {out}"
    );
    assert!(
        !out.starts_with(" 1:"),
        "text template should not include line numbers: {out}"
    );
}

#[test]
fn code_format_override_json_via_stdin() {
    let data = std::fs::read_to_string("tests/fixtures/code/sample.py")
        .expect("read fixture");
    let out = common::run_cli(
        &["--no-color", "-c", "120", "-i", "text", "-f", "json"],
        Some(data.as_bytes()),
    );
    let out = out.stdout;
    assert!(
        out.trim_start().starts_with('['),
        "expected JSON array output, got: {out}"
    );
}
