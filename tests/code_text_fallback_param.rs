use insta::assert_snapshot;
use std::path::Path;
use test_each_file::test_each_path;

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

fn run_cli_text_ingest_json_format_with_style(
    path: &Path,
    style: &str,
) -> String {
    let assert = assert_cmd::cargo::cargo_bin_cmd!("headson")
        .args([
            "--no-color",
            "-c",
            "120",
            // Force text ingest but render with JSON template
            "-i",
            "text",
            "-f",
            "json",
            "-t",
            style,
            path.to_str().unwrap(),
        ])
        .assert()
        .success();

    let mut out =
        String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    while out.ends_with('\n') {
        out.pop();
    }
    out.push('\n');
    out
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
        let out = run_cli_auto_text_with_style(path, style);
        assert_snapshot!(
            format!("code_text_fallback_{}_{}", name, style),
            out
        );
        // Also snapshot the underlying structure via JSON serialization using text ingest
        let out_json = run_cli_text_ingest_json_format_with_style(path, style);
        assert_snapshot!(
            format!("code_text_fallback_{}_{}_json", name, style),
            out_json
        );
    }
}
