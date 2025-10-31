use std::fs;

#[test]
fn single_file_auto_handles_yml_and_uppercase_extensions() {
    let dir = tempfile::tempdir().expect("tmpdir");
    let p1 = dir.path().join("data.yml");
    let p2 = dir.path().join("UPPER.YAML");
    fs::write(&p1, b"k: 1\n").unwrap();
    fs::write(&p2, b"x: 2\n").unwrap();

    for p in [&p1, &p2] {
        let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("headson");
        let assert = cmd
            .args([
                "--no-color",
                "-n",
                "10000",
                "-f",
                "auto",
                p.to_str().unwrap(),
            ])
            .assert()
            .success();
        let out = String::from_utf8_lossy(&assert.get_output().stdout);
        assert!(out.contains(":"), "expected YAML mapping syntax: {out:?}");
    }
}
