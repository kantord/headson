#[path = "../test_support/mod.rs"]
mod util;
use std::fs;

fn run_truncated_string(input: &str, template: &str, cap: usize) -> String {
    let cap_s = cap.to_string();
    let out = util::run_template_budget(
        input,
        template,
        1000, // generous byte budget; truncation driven by --string-cap
        &["--string-cap", &cap_s],
    );
    serde_json::from_str::<String>(&out)
        .expect("output should be a JSON string")
}

fn load_fixture(path: &str) -> String {
    fs::read_to_string(path).expect("read unicode fixture")
}

#[test]
fn unicode_emoji_skin_tone_truncates_on_grapheme_boundary() {
    // 👍🏽 is a single grapheme (thumbs up + medium skin tone)
    let json =
        load_fixture("tests/fixtures/explicit/unicode_emoji_skin_tone.json");
    let expected = "👍🏽👍🏽…".to_string();
    for tmpl in ["json", "pseudo", "js"] {
        let out = run_truncated_string(&json, tmpl, 2);
        assert_eq!(out, expected, "template={tmpl}");
    }
}

#[test]
fn unicode_zwj_family_truncates_on_grapheme_boundary() {
    // Family: 👨‍👩‍👧‍👦 (multiple codepoints joined by ZWJ) repeated twice
    let json = load_fixture("tests/fixtures/explicit/unicode_zwj_family.json");
    let expected = "👨‍👩‍👧‍👦…".to_string();
    for tmpl in ["json", "pseudo", "js"] {
        let out = run_truncated_string(&json, tmpl, 1);
        assert_eq!(out, expected, "template={tmpl}");
    }
}

#[test]
fn unicode_combining_marks_truncate_as_whole_graphemes() {
    // Use decomposed e + combining acute accent (U+0301), repeated three times
    let json = load_fixture("tests/fixtures/explicit/unicode_combining.json");
    let expected = "e\u{0301}e\u{0301}…".to_string(); // éé…
    for tmpl in ["json", "pseudo", "js"] {
        let out = run_truncated_string(&json, tmpl, 2);
        assert_eq!(out, expected, "template={tmpl}");
    }
}

#[test]
fn unicode_flag_regional_indicators_truncate_on_pairs() {
    // Flag: 🇺🇳 is formed by two regional indicator symbols; repeat three times
    let json = load_fixture("tests/fixtures/explicit/unicode_flags.json");
    let expected = "🇺🇳🇺🇳…".to_string();
    for tmpl in ["json", "pseudo", "js"] {
        let out = run_truncated_string(&json, tmpl, 2);
        assert_eq!(out, expected, "template={tmpl}");
    }
}
