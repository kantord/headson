use insta::assert_snapshot;
use std::path::Path;
use test_each_file::test_each_path;

#[allow(dead_code, reason = "legacy helper kept during --debug migration")]
fn run_cli_auto_text_with_style(path: &Path, style: &str) -> String {
    let assert = assert_cmd::cargo::cargo_bin_cmd!("hson")
        .args([
            "--no-color",
            "-c",
            "120", // modest budget to trigger omission markers where applicable
            "-f",
            "auto", // for non-json/yaml, this maps to text template
            "-t",
            style, // strict | default | detailed
            path.to_str().unwrap(),
        ])
        .assert()
        .success();

    let mut out =
        String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    // Normalize trailing newlines to a single one to keep snapshots stable.
    while out.ends_with('\n') {
        out.pop();
    }
    out.push('\n');
    out
}

fn run_cli_auto_text_with_debug(path: &Path, style: &str) -> (String, String) {
    let assert = assert_cmd::cargo::cargo_bin_cmd!("hson")
        .args([
            "--no-color",
            "--debug",
            "-c",
            "120", // modest budget to trigger omission markers where applicable
            "-f",
            "auto", // for non-json/yaml, this maps to text template
            "-t",
            style, // strict | default | detailed
            path.to_str().unwrap(),
        ])
        .assert()
        .success();

    let mut out =
        String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    // Normalize trailing newlines to a single one to keep snapshots stable.
    while out.ends_with('\n') {
        out.pop();
    }
    out.push('\n');

    let err = String::from_utf8_lossy(&assert.get_output().stderr).to_string();
    let norm = normalize_debug(&err);
    (out, norm)
}

// Normalizer to stabilize the full pruned debug JSON tree across runs.
// Mirrors tests/debug_snapshots.rs: zeroes volatile ids and count totals but
// keeps full children so we can debug structure.
fn normalize_debug(s: &str) -> String {
    use serde_json::Value;
    fn zero_ids_and_counts(map: &mut serde_json::Map<String, Value>) {
        if let Some(id) = map.get_mut("id") {
            *id = Value::from(0);
        }
        if let Some(counts) = map.get_mut("counts") {
            if let Some(obj) = counts.as_object_mut() {
                obj.insert("total_nodes".to_string(), Value::from(0));
                obj.insert("included".to_string(), Value::from(0));
            }
        }
    }
    fn walk(v: &mut Value) {
        match v {
            Value::Object(map) => {
                zero_ids_and_counts(map);
                map.iter_mut().for_each(|(_, vv)| walk(vv));
            }
            Value::Array(arr) => arr.iter_mut().for_each(walk),
            _ => {}
        }
    }
    let mut v: Value = serde_json::from_str(s).expect("stderr must be JSON");
    walk(&mut v);
    serde_json::to_string_pretty(&v).unwrap()
}

fn stem_str(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string()
}

fn stem_with_ext(path: &Path) -> String {
    let stem = stem_str(path);
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if ext.is_empty() {
        stem
    } else {
        format!("{stem}_{ext}")
    }
}

fn is_code_file(path: &Path) -> bool {
    match path.extension().and_then(|e| e.to_str()) {
        Some(ext) => matches!(
            ext,
            // include common code sample extensions we added
            "cpp"
                | "cc"
                | "cxx"
                | "py"
                | "java"
                | "js"
                | "ts"
                | "tsx"
                | "go"
                | "sh"
        ),
        None => false,
    }
}

test_each_path! { in "tests/fixtures/code" => code_text_fallback_case }

fn code_text_fallback_case(path: &Path) {
    if !is_code_file(path) {
        return;
    }
    let name = stem_with_ext(path);
    // Single canonical snapshot for Code template (style has no effect on output).
    let (out_default, _err_dbg_default) =
        run_cli_auto_text_with_debug(path, "default");
    assert_snapshot!(
        format!("code_text_fallback_{}_stdout", name),
        out_default
    );

    // Assert style invariance of STDOUT for code template.
    let out_strict = run_cli_auto_text_with_style(path, "strict");
    let out_detailed = run_cli_auto_text_with_style(path, "detailed");
    assert_eq!(
        out_strict, out_default,
        "strict vs default differ for {name}"
    );
    assert_eq!(
        out_detailed, out_default,
        "detailed vs default differ for {name}"
    );
}
