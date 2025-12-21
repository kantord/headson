mod common;
use insta::assert_snapshot;
use std::path::Path;
use test_each_file::test_each_path;

#[allow(dead_code, reason = "legacy helper kept during --debug migration")]
fn run_cli_auto_text_with_style(path: &Path, style: &str) -> String {
    let output = common::run_cli(
        &[
            "--no-color",
            "-c",
            "120", // modest budget to trigger omission markers where applicable
            "-f",
            "auto", // for non-json/yaml, this maps to text template
            "-t",
            style, // strict | default | detailed
            path.to_str().unwrap(),
        ],
        None,
    );

    let out = output.stdout;
    common::normalize_trailing_newline(&out)
}

fn run_cli_auto_text_with_debug(path: &Path, style: &str) -> (String, String) {
    let output = common::run_cli(
        &[
            "--no-color",
            "--debug",
            "-c",
            "120", // modest budget to trigger omission markers where applicable
            "-f",
            "auto", // for non-json/yaml, this maps to text template
            "-t",
            style, // strict | default | detailed
            path.to_str().unwrap(),
        ],
        None,
    );

    let out = output.stdout;
    let out = common::normalize_trailing_newline(&out);

    let err = output.stderr;
    let norm = common::normalize_debug(&err);
    (out, norm)
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
