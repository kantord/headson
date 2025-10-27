use assert_cmd::Command;
use insta::assert_snapshot;

#[test]
fn pseudo_color_string_value_snapshot() {
    let input = b"\"hello\""; // JSON string
    let mut cmd = Command::cargo_bin("headson").expect("bin");
    let assert = cmd
        .args(["--color", "-n", "1000", "-f", "pseudo"]) // force color
        .write_stdin(input.as_slice())
        .assert()
        .success();
    let out =
        String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    assert_snapshot!("pseudo_color_string_value", out);
}

#[test]
fn pseudo_color_object_key_and_value_snapshot() {
    let input = b"{\"k\":\"v\"}"; // simple object
    let mut cmd = Command::cargo_bin("headson").expect("bin");
    let assert = cmd
        .args(["--color", "-n", "1000", "-f", "pseudo"]) // force color
        .write_stdin(input.as_slice())
        .assert()
        .success();
    let out =
        String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    assert_snapshot!("pseudo_color_object_key_and_value", out);
}

#[test]
fn pseudo_no_color_has_no_escape_sequences() {
    let input = b"{\"k\":\"v\"}";
    let mut cmd = Command::cargo_bin("headson").expect("bin");
    let assert = cmd
        .args(["--no-color", "-n", "1000", "-f", "pseudo"]) // force no color
        .write_stdin(input.as_slice())
        .assert()
        .success();
    let out =
        String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    assert!(
        !out.contains('\u{001b}'),
        "output should contain no ANSI escapes: {out:?}"
    );
}
