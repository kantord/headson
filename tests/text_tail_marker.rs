#[test]
fn text_tail_places_marker_at_start() {
    // Use default style ("…") and tail mode; expect marker at the beginning.
    let input = (0..30)
        .map(|i| format!("line{i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let assert = assert_cmd::cargo::cargo_bin_cmd!("headson")
        .args([
            "--no-color",
            "--tail",
            "-i",
            "text",
            "-f",
            "text",
            "-n",
            "30",
        ]) // smallish budget
        .write_stdin(input)
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);
    let mut lines = out.lines();
    let first = lines.next().unwrap_or("");
    assert_eq!(
        first, "…",
        "tail mode should place omission at start: {out:?}"
    );
    // Ensure no omission marker at the end.
    let last = out
        .trim_end_matches('\n')
        .rsplit_once('\n')
        .map(|(_, s)| s)
        .unwrap_or(first);
    assert_ne!(
        last, "…",
        "tail mode should not place omission at end: {out:?}"
    );
}
