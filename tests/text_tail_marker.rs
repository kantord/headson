mod common;

#[test]
fn text_tail_places_marker_at_start() {
    // Use default style ("…") and tail mode; expect marker at the beginning.
    let input = (0..30)
        .map(|i| format!("line{i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let out = common::run_cli(
        &[
            "--no-color",
            "--tail",
            "-i",
            "text",
            "-f",
            "text",
            "-c",
            "30",
        ], // smallish budget
        Some(input.as_bytes()),
    );
    assert!(out.status.success(), "cli should succeed");
    let out = String::from_utf8_lossy(&out.stdout);
    let mut lines = out.lines();
    let first = lines.next().unwrap_or("");
    assert_eq!(
        first, "…",
        "tail mode should place omission at start: {out:?}"
    );
    // Ensure no omission marker at the end.
    let last = common::trim_trailing_newlines(&out)
        .rsplit_once('\n')
        .map(|(_, s)| s)
        .unwrap_or(first);
    assert_ne!(
        last, "…",
        "tail mode should not place omission at end: {out:?}"
    );
}
