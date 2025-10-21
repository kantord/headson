#[path = "../test_support/mod.rs"]
mod util;

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

#[test]
fn unicode_emoji_skin_tone_truncates_on_grapheme_boundary() {
    // 👍🏽 is a single grapheme (thumbs up + medium skin tone)
    let s = "\u{1F44D}\u{1F3FD}\u{1F44D}\u{1F3FD}\u{1F44D}\u{1F3FD}"; // 👍🏽👍🏽👍🏽
    let json = format!("\"{}\"", s); // root string
    let expected = "👍🏽👍🏽…".to_string();
    for tmpl in ["json", "pseudo", "js"] {
        let out = run_truncated_string(&json, tmpl, 2);
        assert_eq!(out, expected, "template={tmpl}");
    }
}

#[test]
fn unicode_zwj_family_truncates_on_grapheme_boundary() {
    // Family: 👨‍👩‍👧‍👦 (multiple codepoints joined by ZWJ) repeated twice
    let s = "👨\u{200D}👩\u{200D}👧\u{200D}👦👨\u{200D}👩\u{200D}👧\u{200D}👦";
    let json = format!("\"{}\"", s);
    let expected = "👨‍👩‍👧‍👦…".to_string();
    for tmpl in ["json", "pseudo", "js"] {
        let out = run_truncated_string(&json, tmpl, 1);
        assert_eq!(out, expected, "template={tmpl}");
    }
}

#[test]
fn unicode_combining_marks_truncate_as_whole_graphemes() {
    // Use decomposed e + combining acute accent (U+0301), repeated three times
    let s = "e\u{0301}e\u{0301}e\u{0301}"; // ééé
    let json = format!("\"{}\"", s);
    let expected = "e\u{0301}e\u{0301}…".to_string(); // éé…
    for tmpl in ["json", "pseudo", "js"] {
        let out = run_truncated_string(&json, tmpl, 2);
        assert_eq!(out, expected, "template={tmpl}");
    }
}

#[test]
fn unicode_flag_regional_indicators_truncate_on_pairs() {
    // Flag: 🇺🇳 is formed by two regional indicator symbols; repeat three times
    let s = "\u{1F1FA}\u{1F1F3}\u{1F1FA}\u{1F1F3}\u{1F1FA}\u{1F1F3}"; // 🇺🇳🇺🇳🇺🇳
    let json = format!("\"{}\"", s);
    let expected = "🇺🇳🇺🇳…".to_string();
    for tmpl in ["json", "pseudo", "js"] {
        let out = run_truncated_string(&json, tmpl, 2);
        assert_eq!(out, expected, "template={tmpl}");
    }
}
