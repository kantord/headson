use std::fs;
use std::path::Path;

use assert_cmd::Command;
use insta::assert_snapshot;

fn run_case(path: &Path, template: &str, n: u32) -> String {
    let input = fs::read_to_string(path).expect("read fixture");
    let mut cmd = Command::cargo_bin("headson").expect("bin");
    let output = cmd
        .arg("-n")
        .arg(n.to_string())
        .arg("-f")
        .arg(template)
        .write_stdin(input)
        .output()
        .expect("run");
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn e2e_parametric() {
    let dir = Path::new("tests/e2e_inputs");
    let templates = ["json", "pseudo", "js"];
    let budgets = [10u32, 100u32, 250u32, 1000u32, 10000u32];
    for entry in fs::read_dir(dir).expect("list dir") {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_file() {
            let name = entry.file_name().into_string().unwrap();
            for &n in &budgets {
                for tmpl in templates {
                    let stdout = run_case(&entry.path(), tmpl, n);
                    assert_snapshot!(
                        format!(
                            "e2e_{}__{}__n{}",
                            name.replace('.', "_"),
                            tmpl,
                            n
                        ),
                        stdout
                    );
                }
            }
        }
    }
}
