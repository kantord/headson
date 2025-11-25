use assert_cmd::cargo::cargo_bin_cmd;

// TDD: start with expectations for strong --grep behavior.
// Weak mode will be added later; these cover the guaranteed inclusion path.

#[test]
fn grep_guarantees_match_even_when_budget_is_tiny() {
    let input = br#"{"outer":{"inner":"needle"},"other":"zzzz"}"#.to_vec();
    let assert = cargo_bin_cmd!("hson")
        .args([
            "--no-color",
            "--bytes",
            "5",
            "-f",
            "json",
            "-t",
            "strict",
            "--grep",
            "needle",
        ])
        .write_stdin(input)
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("needle"),
        "match should be present even if it pushes past the user budget"
    );
    assert!(
        stdout.len() > 5,
        "effective budget should grow to fit the must-keep closure"
    );
}

#[test]
fn grep_keeps_ancestor_path_for_matches() {
    let input = br#"{"outer":{"inner":{"value":"match-me"}}}"#.to_vec();
    let assert = cargo_bin_cmd!("hson")
        .args([
            "--no-color",
            "-c",
            "8",
            "-f",
            "json",
            "-t",
            "strict",
            "--grep",
            "match-me",
        ])
        .write_stdin(input)
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("match-me"),
        "matched leaf should always appear"
    );
    assert!(
        stdout.contains("outer") && stdout.contains("inner"),
        "ancestors of matched nodes should be kept so structure remains navigable"
    );
}

#[test]
fn grep_pins_sampled_array_elements() {
    let input = br#"[{"id":1},{"id":2},{"id":3,"value":"NEEDLE"},{"id":4,"value":"skip-me"}]"#.to_vec();
    let assert = cargo_bin_cmd!("hson")
        .args([
            "--no-color",
            "--bytes",
            "12",
            "-f",
            "json",
            "-t",
            "strict",
            "--grep",
            "NEEDLE",
        ])
        .write_stdin(input)
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("NEEDLE"),
        "array sampling should not drop matched elements in strong grep mode"
    );
    assert!(
        stdout.len() > 12,
        "strong grep should expand the effective budget beyond the user cap to include matches"
    );
}
