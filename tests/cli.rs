use assert_cmd::Command;

#[test]
fn prints_empty_array_compact() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("headson")?;
    cmd.arg("-f").arg("json")
        .write_stdin("[]")
        .assert()
        .success()
        .stdout("[]\n");
    Ok(())
}

#[test]
fn prints_empty_object_compact() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("headson")?;
    cmd.arg("-f").arg("json")
        .write_stdin("{}")
        .assert()
        .success()
        .stdout("{}\n");
    Ok(())
}

#[test]
fn budget_10_and_single_15_char_string_returns_js_comment_array() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("headson")?;
    cmd.arg("-f").arg("js").arg("-n").arg("10")
        .write_stdin("[\"123456789012345\"]")
        .assert()
        .success()
        .stdout("[\n  /* 1 more item */\n]\n");
    Ok(())
}

#[test]
fn pseudo_budget_10_and_single_15_char_string_prints_ellipsis_array() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("headson")?;
    cmd.arg("-f").arg("pseudo").arg("-n").arg("10")
        .write_stdin("[\"123456789012345\"]")
        .assert()
        .success()
        .stdout("[\n  …\n]\n");
    Ok(())
}

#[test]
fn pseudo_budget_50_and_single_15_char_string_returns_entire_array() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("headson")?;
    cmd.arg("-f").arg("pseudo").arg("-n").arg("50")
        .write_stdin("[\"123456789012345\"]")
        .assert()
        .success()
        .stdout("[\n  \"123456789012345\"\n]\n");
    Ok(())
}

#[test]
fn budget_50_and_single_15_char_string_returns_entire_array() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("headson")?;
    cmd.arg("-f").arg("json").arg("-n").arg("50")
        .write_stdin("[\"123456789012345\"]")
        .assert()
        .success()
        .stdout("[\n  \"123456789012345\"\n]\n");
    Ok(())
}
