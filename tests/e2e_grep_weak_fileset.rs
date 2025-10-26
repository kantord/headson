use assert_cmd::Command;
use std::fs;

fn run_paths(args: &[&str]) -> String {
    let mut cmd = Command::cargo_bin("headson").expect("bin");
    let out = cmd
        .args(args)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    String::from_utf8_lossy(&out).into_owned()
}

fn index_of(hay: &str, needle: &str) -> Option<usize> {
    hay.find(needle)
}

#[test]
#[allow(
    clippy::cognitive_complexity,
    reason = "single E2E test covers setup + order checks"
)]
fn e2e_grep_weak_fileset_js_orders_matching_first() {
    let tmp = tempfile::tempdir().expect("tmp");
    let a = tmp.path().join("a.json");
    let b = tmp.path().join("b.json");
    fs::write(&a, b"{\n  \"k\": \"foo\"\n}").unwrap();
    fs::write(&b, b"{\n  \"k\": \"llibre\"\n}").unwrap();

    // Baseline: expect alpha-first ordering (a.json before b.json)
    let out_base = run_paths(&[
        "-n",
        "60",
        "-f",
        "js",
        a.to_str().unwrap(),
        b.to_str().unwrap(),
    ]);
    let ha = format!("// {}", a.display());
    let hb = format!("// {}", b.display());
    let a_pos_base = index_of(&out_base, &ha);
    let b_pos_base = index_of(&out_base, &hb);
    assert!(
        a_pos_base.is_some(),
        "baseline should include a.json header: {out_base}"
    );
    // Not asserting on b presence in baseline (may or may not fit), but if present, must follow a
    if let (Some(ia), Some(ib)) = (a_pos_base, b_pos_base) {
        assert!(
            ia < ib,
            "baseline alpha order expected: a before b: {out_base}"
        );
    }

    // With grep-weak, matching b.json should appear before a.json when present.
    let out_grep = run_paths(&[
        "-n",
        "60",
        "-f",
        "js",
        "--grep-weak",
        "llibre",
        a.to_str().unwrap(),
        b.to_str().unwrap(),
    ]);
    let a_pos = index_of(&out_grep, &ha);
    let b_pos = index_of(&out_grep, &hb);
    assert!(
        b_pos.is_some(),
        "grep-weak should include b.json header: {out_grep}"
    );
    if let (Some(ia), Some(ib)) = (a_pos, b_pos) {
        assert!(
            ib < ia,
            "grep-weak should order matching b before a: {out_grep}"
        );
    }
}

#[test]
#[allow(
    clippy::cognitive_complexity,
    reason = "single E2E test covers setup + order checks"
)]
fn e2e_grep_weak_fileset_pseudo_orders_matching_first() {
    let tmp = tempfile::tempdir().expect("tmp");
    let a = tmp.path().join("a.json");
    let b = tmp.path().join("b.json");
    fs::write(&a, b"{\n  \"k\": \"foo\"\n}").unwrap();
    fs::write(&b, b"{\n  \"k\": \"llibre\"\n}").unwrap();

    let out_base = run_paths(&[
        "-n",
        "60",
        "-f",
        "pseudo",
        a.to_str().unwrap(),
        b.to_str().unwrap(),
    ]);
    let ha = format!("==> {} <==", a.display());
    let hb = format!("==> {} <==", b.display());
    let a_pos_base = index_of(&out_base, &ha);
    let b_pos_base = index_of(&out_base, &hb);
    assert!(
        a_pos_base.is_some(),
        "baseline should include a header: {out_base}"
    );
    if let (Some(ia), Some(ib)) = (a_pos_base, b_pos_base) {
        assert!(
            ia < ib,
            "baseline alpha order expected: a before b: {out_base}"
        );
    }

    let out_grep = run_paths(&[
        "-n",
        "60",
        "-f",
        "pseudo",
        "--grep-weak",
        "llibre",
        a.to_str().unwrap(),
        b.to_str().unwrap(),
    ]);
    let a_pos = index_of(&out_grep, &ha);
    let b_pos = index_of(&out_grep, &hb);
    assert!(
        b_pos.is_some(),
        "grep-weak should include b header: {out_grep}"
    );
    if let (Some(ia), Some(ib)) = (a_pos, b_pos) {
        assert!(
            ib < ia,
            "grep-weak should order matching b before a: {out_grep}"
        );
    }
}
