mod common;

#[test]
fn text_strict_truncates_without_marker() {
    // Ten short lines; use a tiny budget to force truncation.
    let input = (0..10)
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
            "strict",
            "-c",
            "20",
        ], // small budget
        Some(input.as_bytes()),
    );
    let out = out.stdout;
    // No standalone omission marker line in strict mode (array truncation marker)
    let has_omission_line = out
        .lines()
        .any(|l| l.trim() == "…" || l.contains(" more lines "));
    assert!(
        !has_omission_line,
        "strict text mode should not emit array omission markers: {out:?}"
    );
    // Should be truncated: last line should not appear
    assert!(
        !out.contains("line9\n"),
        "expected truncation under small budget in strict mode: {out:?}"
    );
}
