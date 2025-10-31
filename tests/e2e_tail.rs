use std::fs;
use std::path::Path;

use insta::assert_snapshot;

fn run_case_with_tail(path: &Path, template: &str, n: u32) -> String {
    let input = fs::read_to_string(path).expect("read fixture");
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("headson");
    let output = cmd
        .arg("--no-color")
        .arg("-n")
        .arg(n.to_string())
        .arg("-f")
        .arg(template)
        .arg("--tail")
        .write_stdin(input)
        .output()
        .expect("run");
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn assert_tail_snapshots_for(
    dir: &Path,
    name: &str,
    budgets: &[u32],
    templates: &[&str],
) {
    let path = dir.join(name);
    for &n in budgets {
        for &tmpl in templates {
            let stdout = run_case_with_tail(&path, tmpl, n);
            assert_snapshot!(
                format!(
                    "e2e_tail_{}__{}__n{}",
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
fn e2e_tail_parametric_targeted() {
    let dir = Path::new("tests/fixtures/parametric");
    // Limit to fixtures where tail is meaningful and formatting-rich.
    let files = [
        "simple_array.json",
        "mixed_arrays.json",
        "complex_nested.json",
    ];
    // Focus budgets to keep snapshots concise. For complex_nested, also
    // include a larger budget (1000) to exercise deeper tail formatting used
    // in existing snapshots.
    let budgets_base = [30u32, 200u32];
    // Tail affects visual markers in Pseudo/JS; JSON remains strict JSON and is
    // verified separately below.
    let templates = ["pseudo", "js"];
    for name in files {
        let mut budgets: Vec<u32> = budgets_base.to_vec();
        if name == "complex_nested.json" {
            budgets.push(1000);
        }
        assert_tail_snapshots_for(dir, name, &budgets, &templates);
    }
}

#[test]
fn e2e_tail_json_remains_strict() {
    // Single sanity check: JSON template remains valid and unannotated under --tail.
    let path = Path::new("tests/fixtures/parametric/simple_array.json");
    let out = run_case_with_tail(path, "json", 30);
    let v: serde_json::Value = serde_json::from_str(&out).expect("json parse");
    assert!(v.is_array() || v.is_object());
    assert!(!out.contains('…'));
    assert!(!out.contains("/*"));
}
