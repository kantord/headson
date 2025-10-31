use std::fs::{self, File};
use std::io::Write as _;

fn with_tmp<F: FnOnce(&std::path::Path)>(f: F) {
    let td = tempfile::tempdir().expect("tempdir");
    f(td.path());
}

#[test]
#[allow(
    clippy::cognitive_complexity,
    reason = "Test assembles temp files and multi-step assertions concisely."
)]
fn user_selected_json_overrides_yaml_detection() {
    with_tmp(|dir| {
        // Write a file with .yaml extension but JSON content.
        let p_yaml = dir.join("sample.yaml");
        let mut f = File::create(&p_yaml).expect("create");
        writeln!(f, "{{\"k\": 1}}\n").expect("write");

        // And a normal JSON-named file.
        let p_json = dir.join("data.json");
        fs::write(&p_json, b"{\n  \"x\": 2\n}\n").expect("write json");

        let budget = 10_000usize.to_string();
        let assert = assert_cmd::cargo::cargo_bin_cmd!("headson")
            .args([
                "--no-color",
                "-n",
                &budget,
                "-f",
                "json",
                p_yaml.to_str().unwrap(),
                p_json.to_str().unwrap(),
            ])
            .assert()
            .success();
        let out = String::from_utf8_lossy(&assert.get_output().stdout);
        // The body under the sample.yaml header should be JSON-style.
        assert!(out.contains("sample.yaml"));
        // Find the header line containing sample.yaml and inspect following text.
        let mut lines = out.lines();
        let mut found = false;
        while let Some(line) = lines.next() {
            if line.starts_with("==> ") && line.contains("sample.yaml") {
                // Next lines should include JSON braces soon after.
                let rest: String = lines.collect::<Vec<_>>().join("\n");
                assert!(
                    rest.contains('{'),
                    "expected JSON body after header: {out:?}"
                );
                found = true;
                break;
            }
        }
        assert!(found, "header not found in output: {out:?}");
    });
}

#[test]
#[allow(
    clippy::cognitive_complexity,
    reason = "Test assembles temp files and multi-step assertions concisely."
)]
fn user_selected_yaml_overrides_json_detection() {
    with_tmp(|dir| {
        let p1 = dir.join("thing.json");
        fs::write(&p1, b"{\n  \"k\": 1\n}\n").expect("write");
        let p2 = dir.join("aux.json");
        fs::write(&p2, b"{\n  \"q\": 2\n}\n").expect("write");

        let budget = 10_000usize.to_string();
        let assert = assert_cmd::cargo::cargo_bin_cmd!("headson")
            .args([
                "--no-color",
                "-n",
                &budget,
                "-f",
                "yaml",
                p1.to_str().unwrap(),
                p2.to_str().unwrap(),
            ])
            .assert()
            .success();
        let out = String::from_utf8_lossy(&assert.get_output().stdout);
        // YAML object body should contain 'k:' key notation under the header.
        assert!(out.contains("thing.json"));
        let mut lines = out.lines();
        let mut found = false;
        while let Some(line) = lines.next() {
            if line.starts_with("==> ") && line.contains("thing.json") {
                let rest: String = lines.collect::<Vec<_>>().join("\n");
                assert!(
                    rest.contains("k:"),
                    "expected YAML body after header: {out:?}"
                );
                found = true;
                break;
            }
        }
        assert!(found, "header not found in output: {out:?}");
    });
}
