#[test]
fn cpp_text_fallback_snapshot() {
    // Use a real C++-like file with indentation so future changes to
    // text fallback (e.g., indent-aware rendering) will reflect in the snapshot.
    let fixture = std::path::Path::new("tests/fixtures/code/sample.cpp");

    let assert = assert_cmd::cargo::cargo_bin_cmd!("hson")
        .args([
            "--no-color", // stabilize output
            "-c",
            "120", // modest char budget to potentially trigger omission markers
            "-f",
            "auto", // unknown ext => text template fallback
            fixture.to_str().unwrap(),
        ])
        .assert()
        .success();

    let mut out =
        String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    // Normalize trailing newlines to a single one for snapshot stability.
    while out.ends_with('\n') {
        out.pop();
    }
    out.push('\n');

    insta::assert_snapshot!(out);
}

#[test]
fn cpp_text_fallback_snapshot_json() {
    let fixture = std::path::Path::new("tests/fixtures/code/sample.cpp");
    let assert = assert_cmd::cargo::cargo_bin_cmd!("hson")
        .args([
            "--no-color",
            "-c",
            "120",
            // Force text ingest, but render with JSON template for structure visibility
            "-i",
            "text",
            "-f",
            "json",
            fixture.to_str().unwrap(),
        ])
        .assert()
        .success();

    let mut out =
        String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    while out.ends_with('\n') {
        out.pop();
    }
    out.push('\n');
    insta::assert_snapshot!(out);
}

#[test]
fn code_format_override_text_template() {
    let fixture = std::path::Path::new("tests/fixtures/code/sample.py");
    let assert = assert_cmd::cargo::cargo_bin_cmd!("hson")
        .args([
            "--no-color",
            "-c",
            "120",
            "-i",
            "text",
            "-f",
            "text",
            fixture.to_str().unwrap(),
        ])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);
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
    let assert = assert_cmd::cargo::cargo_bin_cmd!("hson")
        .args(["--no-color", "-c", "120", "-i", "text", "-f", "json"])
        .write_stdin(data)
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        out.trim_start().starts_with('['),
        "expected JSON array output, got: {out}"
    );
}
