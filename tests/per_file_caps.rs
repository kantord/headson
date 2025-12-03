use assert_cmd::cargo::cargo_bin_cmd;
use std::collections::HashMap;
use std::fs;

fn parse_fileset_sections(output: &str) -> HashMap<String, String> {
    let mut sections: HashMap<String, String> = HashMap::new();
    let mut current: Option<String> = None;
    let mut buf = String::new();
    for line in output.lines() {
        if let Some(name) =
            line.strip_prefix("==> ").and_then(|rest| rest.strip_suffix(" <=="))
        {
            if let Some(key) = current.take() {
                sections.insert(key, buf.clone());
                buf.clear();
            }
            current = Some(name.to_string());
            continue;
        }
        if line.starts_with("==> ") && line.ends_with(" more files <==") {
            // Summary line; ignore.
            continue;
        }
        if current.is_some() {
            if !buf.is_empty() {
                buf.push('\n');
            }
            buf.push_str(line);
        }
    }
    if let Some(key) = current {
        sections.insert(key, buf);
    }
    sections
}

#[test]
fn fileset_respects_per_file_byte_cap() {
    let tmp = tempfile::tempdir_in(".").expect("tmp");
    let big_path = tmp.path().join("big.json");
    let small_path = tmp.path().join("small.json");
    let big_body = "a".repeat(240);
    let small_body = "b".repeat(10);
    fs::write(&big_path, format!(r#"{{"big":"{big_body}"}}"#)).unwrap();
    fs::write(&small_path, format!(r#"{{"small":"{small_body}"}}"#)).unwrap();

    let cap = 150usize;
    let mut cmd = cargo_bin_cmd!("hson");
    let assert = cmd
        .args([
            "--no-color",
            "--no-sort",
            "-f",
            "auto",
            "-c",
            &cap.to_string(),
            big_path.to_str().unwrap(),
            small_path.to_str().unwrap(),
        ])
        .current_dir(tmp.path())
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    let sections = parse_fileset_sections(&out);
    let big_key = big_path.to_string_lossy().to_string();
    let small_key = small_path.to_string_lossy().to_string();

    let big_len = sections
        .get(&big_key)
        .map(|s| s.len())
        .unwrap_or_default();
    let small_len = sections
        .get(&small_key)
        .map(|s| s.len())
        .unwrap_or_default();

    assert!(
        big_len <= cap,
        "big file should be capped per file: len={big_len}, cap={cap}, out={out:?}"
    );
    assert!(
        small_len <= cap,
        "small file should not exceed per-file cap: len={small_len}, cap={cap}, out={out:?}"
    );
}

#[test]
fn fileset_respects_per_file_line_cap_with_counted_headers() {
    let tmp = tempfile::tempdir_in(".").expect("tmp");
    let a = tmp.path().join("a.txt");
    fs::write(&a, "a1\na2\na3\na4\n").unwrap();

    // Cap at 4 lines per file; with -H headers count toward the cap, so each
    // file should contribute at most 3 content lines plus its header.
    let mut cmd = cargo_bin_cmd!("hson");
    let assert = cmd
        .args([
            "--no-color",
            "--no-sort",
            "-f",
            "auto",
            "-n",
            "4",
            "-H",
            "--global-lines",
            "10",
            a.to_str().unwrap(),
        ])
        .current_dir(tmp.path())
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    let lines = out.lines().count();
    assert!(
        lines <= 4,
        "output should honor per-file line cap (including header when present): lines={lines}, out={out:?}"
    );
}
