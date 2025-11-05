use insta::assert_snapshot;
use std::path::Path;
use test_each_file::test_each_path;

#[allow(dead_code, reason = "legacy helper kept during --debug migration")]
fn run_cli_auto_text_with_style(path: &Path, style: &str) -> String {
    let assert = assert_cmd::cargo::cargo_bin_cmd!("headson")
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
    let assert = assert_cmd::cargo::cargo_bin_cmd!("headson")
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

// Minimal normalizer to stabilize debug JSON snapshots across internal changes.
// Mirrors tests/debug_snapshots.rs behavior for id/counts fields.
fn normalize_debug(s: &str) -> String {
    use serde_json::{Value, json};
    let v: Value = serde_json::from_str(s).expect("stderr must be JSON");
    fn pick(obj: &Value, path: &[&str]) -> Value {
        let mut cur = obj;
        for k in path {
            cur = &cur[*k];
        }
        cur.clone()
    }
    let children_len =
        v["root"]["children"].as_array().map(Vec::len).unwrap_or(0);
    let gaps_len = v["root"]["gaps"].as_array().map(Vec::len).unwrap_or(0);
    let summary = json!({
        "template": pick(&v, &["template"]),
        "renderer": {
            "template": pick(&v, &["renderer","template"]),
            "style": pick(&v, &["renderer","style"]),
        },
        "counts": {
            "included": 0,
            "total_nodes": 0,
            "omitted_children": pick(&v, &["counts","omitted_children"]),
        },
        "selection": {
            "top_k": pick(&v, &["selection","top_k"]),
        },
        "output_stats": {
            "bytes": pick(&v, &["output_stats","bytes"]),
            "lines": pick(&v, &["output_stats","lines"]),
        },
        "root": {
            "kind": pick(&v, &["root","kind"]),
            "fileset_root": pick(&v, &["root","fileset_root"]),
            "metrics": pick(&v, &["root","metrics"]),
            "children_len": children_len,
            "gaps_len": gaps_len,
        },
    });
    serde_json::to_string_pretty(&summary).unwrap()
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
    for style in ["strict", "default", "detailed"] {
        let (out, err_dbg) = run_cli_auto_text_with_debug(path, style);
        assert_snapshot!(
            format!("code_text_fallback_{}_{}", name, style),
            out
        );
        // Also snapshot the underlying structure via debug JSON on stderr
        assert_snapshot!(
            format!("code_text_fallback_{}_{}__debug", name, style),
            err_dbg
        );
    }
}
