#[test]
fn text_detailed_shows_omitted_count() {
    // Many lines; detailed style should show count: "… N more lines …"
    let input = (0..50)
        .map(|i| format!("line{i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let assert = assert_cmd::cargo::cargo_bin_cmd!("hson")
        .args([
            "--no-color",
            "-i",
            "text",
            "-f",
            "text",
            "-t",
            "detailed",
            "-c",
            "40",
        ])
        .write_stdin(input)
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        out.contains(" more lines "),
        "expected detailed count marker: {out:?}"
    );
    assert!(
        out.contains("…"),
        "expected ellipsis markers present: {out:?}"
    );
}
