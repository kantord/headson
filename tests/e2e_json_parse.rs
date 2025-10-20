use std::fs;
use std::path::Path;

use assert_cmd::Command;

fn run_case(path: &Path, n: u32) -> String {
    let input = fs::read_to_string(path).expect("read fixture");
    let mut cmd = Command::cargo_bin("headson").expect("bin");
    let output = cmd
        .arg("-n")
        .arg(n.to_string())
        .arg("-f")
        .arg("json")
        .write_stdin(input)
        .output()
        .expect("run");
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn e2e_json_is_parsable_for_all_budgets() {
    let dir = Path::new("tests/e2e_inputs");
    let budgets = [10u32, 100u32, 250u32, 1000u32, 10000u32];
    for entry in fs::read_dir(dir).expect("list dir") {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_file() {
            for &n in &budgets {
                let stdout = run_case(&entry.path(), n);
                serde_json::from_str::<serde_json::Value>(&stdout)
                    .expect("json output should parse for all budgets");
            }
        }
    }
}

