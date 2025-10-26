#[path = "../test_support/mod.rs"]
mod util;

#[test]
fn grep_weak_prefers_value_match_over_alpha_order() {
    // Two props: alphabetically, "aaa" would win. With grep-weak matching
    // the value of "zzz", we expect "zzz" to be kept when budget is tight.
    let input = r#"{ "aaa": "foo", "zzz": "llibre" }"#;
    let budget = 30usize; // small so only one property is kept

    let base = util::run_template_budget(input, "js", budget, &["--compact"]);
    assert!(
        base.contains("\"aaa\":"),
        "baseline should pick alpha-first: {base}"
    );
    assert!(
        !base.contains("\"zzz\":"),
        "baseline should omit zzz: {base}"
    );

    let biased = util::run_template_budget(
        input,
        "js",
        budget,
        &["--compact", "--grep-weak", "llibre"],
    );
    assert!(
        biased.contains("\"zzz\":"),
        "grep-weak should bias zzz to appear: {biased}"
    );
    assert!(
        !biased.contains("\"aaa\":"),
        "grep-weak run should keep only one key: {biased}"
    );
}

#[test]
fn grep_weak_prefers_descendant_match() {
    // Values are nested one level deeper under each top-level key.
    // With grep-weak matching a descendant of "zzz", we expect "zzz" to win.
    let input = r#"{ "aaa": {"k": "foo"}, "zzz": {"k": "llibre"} }"#;
    let budget = 30usize;

    let base = util::run_template_budget(input, "js", budget, &["--compact"]);
    assert!(
        base.contains("\"aaa\":"),
        "baseline should pick alpha-first: {base}"
    );
    assert!(
        !base.contains("\"zzz\":"),
        "baseline should omit zzz: {base}"
    );

    let biased = util::run_template_budget(
        input,
        "js",
        budget,
        &["--compact", "--grep-weak", "llibre"],
    );
    assert!(
        biased.contains("\"zzz\":"),
        "grep-weak should bias path with descendant match: {biased}"
    );
    assert!(
        !biased.contains("\"aaa\":"),
        "grep-weak run should keep only one key: {biased}"
    );
}

#[test]
fn grep_weak_key_match_biases_object_key() {
    // Bias on object key names as well.
    let input = r#"{ "libre_item": 1, "aaaa": 2 }"#;
    let budget = 24usize;

    let base = util::run_template_budget(input, "js", budget, &["--compact"]);
    assert!(base.contains("\"aaaa\":"), "baseline alpha-first: {base}");
    assert!(
        !base.contains("\"libre_item\":"),
        "baseline omits libre: {base}"
    );

    let biased = util::run_template_budget(
        input,
        "js",
        budget,
        &["--compact", "--grep-weak", "libre"],
    );
    assert!(
        biased.contains("\"libre_item\":"),
        "grep-weak should bias key name matches: {biased}"
    );
    assert!(
        !biased.contains("\"aaaa\":"),
        "grep-weak run should keep only one key: {biased}"
    );
}
