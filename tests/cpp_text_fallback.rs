#[test]
fn cpp_text_fallback_snapshot() {
    // Use a real C++-like file with indentation so future changes to
    // text fallback (e.g., indent-aware rendering) will reflect in the snapshot.
    let fixture = std::path::Path::new("tests/fixtures/code/sample.cpp");

    let assert = assert_cmd::cargo::cargo_bin_cmd!("headson")
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
