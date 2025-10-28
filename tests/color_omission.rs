use assert_cmd::Command;

#[test]
fn pseudo_ellipsis_is_dark_gray() {
    // Force omission with small budget on an array
    let input = "[1,2,3,4,5,6,7,8,9,10]";
    let mut cmd = Command::cargo_bin("headson").expect("bin");
    let assert = cmd
        .args(["--color", "-n", "10", "-f", "pseudo"]) // small budget
        .write_stdin(input)
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        out.contains("\u{001b}[90m…\u{001b}[0m"),
        "expected dark gray ellipsis in pseudo: {out:?}"
    );
}

#[test]
fn js_omission_comment_is_dark_gray() {
    let input = "[1,2,3,4,5,6,7,8,9,10]";
    let mut cmd = Command::cargo_bin("headson").expect("bin");
    let assert = cmd
        .args(["--color", "-n", "10", "-f", "js"]) // small budget
        .write_stdin(input)
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        out.contains("\u{001b}[90m/* ")
            && out.contains(" more items */\u{001b}[0m"),
        "expected dark gray comment in js: {out:?}"
    );
}
