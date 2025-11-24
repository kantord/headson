use insta::assert_snapshot;
use std::path::PathBuf;

fn run(args: &[&str]) -> String {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("hson");
    let assert = cmd
        .args(
            std::iter::once("--no-color")
                .chain(std::iter::once("--no-sort"))
                .chain(args.iter().copied()),
        )
        .assert()
        .success();
    String::from_utf8_lossy(&assert.get_output().stdout).into_owned()
}

fn fixture_path(name: &str) -> String {
    PathBuf::from("tests/fixtures/mixed_headers")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

#[test]
fn headers_free_by_default_under_char_cap() {
    let out = run(&[
        "-u",
        "50", // per-file => ~150 chars across 3 inputs
        &fixture_path("a.json"),
        &fixture_path("b.yaml"),
        &fixture_path("c.txt"),
    ]);
    let normalized = out.replace('\\', "/");
    assert_snapshot!("mixed_headers__free", normalized);
}

#[test]
fn headers_count_under_char_cap_with_flag() {
    let out = run(&[
        "-u",
        "50",
        "-H",
        &fixture_path("a.json"),
        &fixture_path("b.yaml"),
        &fixture_path("c.txt"),
    ]);
    let normalized = out.replace('\\', "/");
    assert_snapshot!("mixed_headers__counted", normalized);
}
