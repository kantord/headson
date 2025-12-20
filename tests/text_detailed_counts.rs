mod common;

#[test]
fn text_detailed_shows_omitted_count() {
    // Many lines; detailed style should show count: "… N more lines …"
    let input = (0..50)
        .map(|i| format!("line{i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let out = common::run_cli(
        &[
            "--no-color",
            "-i",
            "text",
            "-f",
            "text",
            "-t",
            "detailed",
            "-c",
            "40",
        ],
        Some(input.as_bytes()),
    );
    assert!(out.status.success(), "cli should succeed");
    let out = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.contains(" more lines "),
        "expected detailed count marker: {out:?}"
    );
    assert!(
        out.contains("…"),
        "expected ellipsis markers present: {out:?}"
    );
}
