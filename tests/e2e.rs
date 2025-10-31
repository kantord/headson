use std::fs;
use std::path::Path;

use insta::assert_snapshot;

fn run_case(path: &Path, template: &str, n: u32) -> String {
    let input = fs::read_to_string(path).expect("read fixture");
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("headson");
    let n_s = n.to_string();
    let mut args = vec!["--no-color", "-n", &n_s];
    let lower = template.to_ascii_lowercase();
    match lower.as_str() {
        "json" => args.extend(["-f", "json", "-t", "strict"]),
        "yaml" => args.extend(["-f", "yaml", "-i", "yaml"]),
        "pseudo" => args.extend(["-f", "json", "-t", "default"]),
        "js" => args.extend(["-f", "json", "-t", "detailed"]),
        other => args.extend(["-f", other]),
    }
    let output = cmd.args(args).write_stdin(input).output().expect("run");
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn e2e_parametric() {
    let dir = Path::new("tests/fixtures/parametric");
    let templates = ["json", "pseudo", "js"];
    let budgets = [10u32, 100u32, 250u32, 1000u32, 10000u32];
    for entry in fs::read_dir(dir).expect("list dir") {
        let entry = entry.unwrap();
        if !entry.file_type().unwrap().is_file() {
            continue;
        }
        let name = entry.file_name().into_string().unwrap();
        assert_snapshots_for(&entry.path(), &name, &templates, &budgets);
    }
}

fn assert_snapshots_for(
    path: &Path,
    name: &str,
    templates: &[&str],
    budgets: &[u32],
) {
    for &n in budgets {
        for &tmpl in templates {
            let stdout = run_case(path, tmpl, n);
            assert_snapshot!(
                format!("e2e_{}__{}__n{}", name.replace('.', "_"), tmpl, n),
                stdout
            );
        }
    }
}
