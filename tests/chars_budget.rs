mod common;

fn count_chars_normalized(s: &str) -> usize {
    common::trim_trailing_newlines(s).chars().count()
}

#[test]
fn ascii_parity_with_bytes() {
    // ASCII-only; bytes and chars budgets of the same numeric value should match.
    let input =
        "[\"x\",\"x\",\"x\",\"x\",\"x\",\"x\",\"x\",\"x\",\"x\",\"x\"]";
    let out_c = common::run_cli(
        &["--no-color", "-c", "60", "-f", "json", "-t", "strict"],
        Some(input.as_bytes()),
    ); // bytes
    let out_u = common::run_cli(
        &["--no-color", "-u", "60", "-f", "json", "-t", "strict"],
        Some(input.as_bytes()),
    ); // chars
    let s_c = out_c.stdout;
    let s_u = out_u.stdout;
    assert_eq!(s_c, s_u, "ASCII output should be identical for -c and -u");
}

#[test]
fn multibyte_chars_allow_more_than_bytes_at_same_cap() {
    // Input with multi-byte characters (é). With same numeric cap, --chars can allow
    // more content than --bytes.
    let input = "[\"é\",\"é\",\"é\",\"é\",\"é\",\"é\",\"é\",\"é\",\"é\",\"é\",\"é\",\"é\"]";
    let out_bytes = common::run_cli(
        &["--no-color", "-c", "60", "-f", "json", "-t", "strict"],
        Some(input.as_bytes()),
    ); // bytes
    let out_chars = common::run_cli(
        &["--no-color", "-u", "60", "-f", "json", "-t", "strict"],
        Some(input.as_bytes()),
    ); // chars
    let s_b = out_bytes.stdout;
    let s_u = out_chars.stdout;
    // Compare by final byte lengths as a proxy; char budget should not be shorter.
    assert!(
        s_u.len() >= s_b.len(),
        "expected --chars output length >= --bytes, got chars={} bytes={}\nchars_out={:?}\nbytes_out={:?}",
        s_u.len(),
        s_b.len(),
        s_u,
        s_b
    );
}

#[test]
fn colored_vs_plain_match_after_stripping_under_char_budget() {
    // Arrange a small array whose render sits near the char budget edge.
    let input =
        b"[\"x\",\"x\",\"x\",\"x\",\"x\",\"x\",\"x\",\"x\",\"x\",\"x\",\"x\"]";

    let cfg_plain = headson::RenderConfig {
        template: headson::OutputTemplate::Json,
        indent_unit: "  ".to_string(),
        space: " ".to_string(),
        newline: "\n".to_string(),
        prefer_tail_arrays: false,
        color_mode: headson::ColorMode::On,
        color_enabled: false,
        style: headson::Style::Strict,
        string_free_prefix_graphemes: None,
        debug: false,
        primary_source_name: None,
        show_fileset_headers: true,
        fileset_tree: false,
        count_fileset_headers_in_budgets: false,
        grep_highlight: None,
    };
    let cfg_color = headson::RenderConfig {
        color_enabled: true,
        ..cfg_plain.clone()
    };
    let prio = headson::PriorityConfig::new(usize::MAX, usize::MAX);

    let budgets = headson::Budgets {
        global: Some(headson::Budget {
            kind: headson::BudgetKind::Chars,
            cap: 50,
        }),
        per_slot: Some(headson::Budget {
            kind: headson::BudgetKind::Chars,
            cap: 50,
        }),
    };
    let grep = headson::GrepConfig::default();

    let plain = headson::headson(
        headson::InputKind::Json(input.to_vec()),
        &cfg_plain,
        &prio,
        &grep,
        budgets,
    )
    .expect("plain render under char budget")
    .text;
    let colored = headson::headson(
        headson::InputKind::Json(input.to_vec()),
        &cfg_color,
        &prio,
        &grep,
        budgets,
    )
    .expect("colored render under char budget")
    .text;

    // Ensure char budget enforced on uncolored output
    assert!(plain.chars().count() <= budgets.global.unwrap().cap);
    // Stripping ANSI from colored should match plain logical content
    let colored_stripped = common::strip_ansi(&colored);
    assert_eq!(plain, colored_stripped);
}

#[test]
fn combined_chars_and_lines_caps_rejected() {
    let p = "tests/fixtures/explicit/object_small.json";
    let content = std::fs::read_to_string(p).expect("read fixture");
    let out = common::run_cli_expect_fail(
        &[
            "--no-color",
            "-f",
            "json",
            "-t",
            "default",
            "-n",
            "2",
            "-u",
            "100000",
        ],
        Some(content.as_bytes()),
        None,
    ); // conflicting per-file metrics
    let stderr = out.stderr;
    assert!(
        stderr.contains("only one per-file budget"),
        "expected conflict error for mixed per-file metrics: {stderr}"
    );
}

#[test]
fn fileset_char_budget_scales_with_inputs() {
    use std::fs;
    let tmp = tempfile::tempdir().expect("tmp");
    let a = tmp.path().join("a.json");
    let b = tmp.path().join("b.json");
    fs::write(&a, b"[1,2,3,4,5,6,7,8,9,10]").unwrap();
    fs::write(&b, b"[1,2,3,4,5,6,7,8,9,10]").unwrap();

    let out = common::run_cli(
        &[
            "--no-color",
            "-H",
            "-u",
            "40",
            "-f",
            "auto",
            a.to_str().unwrap(),
            b.to_str().unwrap(),
        ],
        None,
    );
    let out = out.stdout;
    // Total char count should be <= per-file cap * number_of_inputs
    assert!(
        count_chars_normalized(&out) <= 80,
        "fileset char budget not enforced: {} > 80\n{}",
        count_chars_normalized(&out),
        out
    );
}
