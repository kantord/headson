use std::fs;
use std::path::Path;

use assert_cmd::Command;
use insta::assert_snapshot;

fn run_case(path: &Path) -> String {
    let input = fs::read_to_string(path).expect("read fixture");
    let mut cmd = Command::cargo_bin("headson").expect("bin");
    let output = cmd.arg("-n").arg("50").write_stdin(input).output().expect("run");
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn e2e_parametric() {
    let dir = Path::new("tests/e2e_inputs");
    for entry in fs::read_dir(dir).expect("list dir") {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_file() {
            let name = entry.file_name().into_string().unwrap();
            let stdout = run_case(&entry.path());
            assert_snapshot!(format!("e2e_{}", name.replace('.', "_")), stdout);
        }
    }
}
