use std::fs;
use std::path::Path;

use assert_cmd::Command;
use insta::assert_snapshot;

fn run_case_with_head(path: &Path, template: &str, n: u32) -> String {
    let input = fs::read_to_string(path).expect("read fixture");
    let mut cmd = Command::cargo_bin("headson").expect("bin");
    let output = cmd
        .arg("-n")
        .arg(n.to_string())
        .arg("-f")
        .arg(template)
        .arg("--head")
        .write_stdin(input)
        .output()
        .expect("run");
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn assert_head_snapshots_for(
    dir: &Path,
    name: &str,
    budgets: &[u32],
    templates: &[&str],
) {
    let path = dir.join(name);
    for &n in budgets {
        for &tmpl in templates {
            let stdout = run_case_with_head(&path, tmpl, n);
            assert_snapshot!(
                format!(
                    "e2e_head_{}__{}__n{}",
                    name.replace('.', "_"),
                    tmpl,
                    n
                ),
                stdout
            );
        }
    }
}

#[test]
fn e2e_head_parametric_targeted() {
    let dir = Path::new("tests/fixtures/parametric");
    // Limit to fixtures where head is meaningful and formatting-rich.
    let files = [
        "simple_array.json",
        "mixed_arrays.json",
        "complex_nested.json",
    ];
    // Keep snapshots concise.
    let budgets_base = [30u32, 200u32];
    // Head affects visual markers in Pseudo/JS; JSON remains strict JSON and is
    // verified separately below.
    let templates = ["pseudo", "js"];
    for name in files {
        let mut budgets: Vec<u32> = budgets_base.to_vec();
        if name == "complex_nested.json" {
            budgets.push(1000);
        }
        assert_head_snapshots_for(dir, name, &budgets, &templates);
    }
}

#[test]
fn e2e_head_json_remains_strict() {
    // Single sanity check: JSON template remains valid and unannotated under --head.
    let path = Path::new("tests/fixtures/parametric/simple_array.json");
    let out = run_case_with_head(path, "json", 30);
    let v: serde_json::Value = serde_json::from_str(&out).expect("json parse");
    assert!(v.is_array() || v.is_object());
    assert!(!out.contains('…'));
    assert!(!out.contains("/*"));
}
