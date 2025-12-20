mod common;

#[test]
fn pseudo_ellipsis_is_dark_gray() {
    // Force omission with small budget on an array
    let input = "[1,2,3,4,5,6,7,8,9,10]";
    let out = common::run_cli(
        &["--color", "-c", "10", "-f", "json", "-t", "default"], // small budget
        Some(input.as_bytes()),
    );
    assert!(out.status.success(), "cli should succeed");
    let out = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.contains("\u{001b}[90m…\u{001b}[0m"),
        "expected dark gray ellipsis in pseudo: {out:?}"
    );
}

#[test]
fn js_omission_comment_is_dark_gray() {
    let input = "[1,2,3,4,5,6,7,8,9,10]";
    let out = common::run_cli(
        &["--color", "-c", "30", "-f", "json", "-t", "detailed"], // small budget
        Some(input.as_bytes()),
    );
    assert!(out.status.success(), "cli should succeed");
    let out = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.contains("\u{001b}[90m/* ")
            && out.contains(" more items */\u{001b}[0m"),
        "expected dark gray comment in js: {out:?}"
    );
}
