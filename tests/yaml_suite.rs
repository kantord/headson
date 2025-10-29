use assert_cmd::Command;
use std::fs;
use std::path::Path;
use test_each_file::test_each_path;

fn run_cli_yaml(input: &[u8]) -> (bool, String, String) {
    let assert = Command::cargo_bin("headson")
        .unwrap()
        .args(["--no-color", "-n", "1000000", "-f", "yaml", "-i", "yaml"]) // parse YAML, render YAML
        .write_stdin(input)
        .assert();
    let ok = assert.get_output().status.success();
    let out =
        String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    let err =
        String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    (ok, out, err)
}

fn is_yaml_file(path: &Path) -> bool {
    path.extension().map(|e| e == "yaml").unwrap_or(false)
}

test_each_path! { in "yaml-test-suite" => yaml_suite_case }

fn yaml_suite_case(path: &Path) {
    if !is_yaml_file(path) {
        return;
    }
    let input = fs::read(path).expect("read yaml");
    let (ok, out, err) = run_cli_yaml(&input);
    assert!(
        ok,
        "cli should succeed for YAML: {}\nerr: {}",
        path.display(),
        err
    );

    // Output should be valid YAML that parses with yaml-rust2 as at least one document.
    let docs = yaml_rust2::YamlLoader::load_from_str(&out)
        .expect("output should parse via yaml-rust2");
    assert!(
        !docs.is_empty(),
        "expected at least one YAML document in output for {}",
        path.display()
    );
}
