use assert_cmd::Command;

#[test]
fn prints_empty_array_compact() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("headson")?;
    cmd.write_stdin("[]")
        .assert()
        .success()
        .stdout("[]\n");
    Ok(())
}

#[test]
fn prints_empty_object_compact() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("headson")?;
    cmd.write_stdin("{}")
        .assert()
        .success()
        .stdout("{}\n");
    Ok(())
}
