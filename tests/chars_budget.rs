use assert_cmd::assert::Assert;

fn run_cli(input: &str, args: &[&str]) -> Assert {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("hson");
    cmd.arg("--no-color").args(args).write_stdin(input).assert()
}

fn count_lines_normalized(s: &str) -> usize {
    if s.is_empty() {
        return 0;
    }
    let trimmed = s.strip_suffix('\n').unwrap_or(s);
    if trimmed.is_empty() {
        0
    } else {
        trimmed.as_bytes().iter().filter(|&&b| b == b'\n').count() + 1
    }
}

fn count_chars_normalized(s: &str) -> usize {
    s.trim_end_matches('\n').chars().count()
}

fn strip_ansi(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
            i += 2;
            while i < bytes.len() {
                let b = bytes[i];
                i += 1;
                if b == b'm' {
                    break;
                }
            }
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).expect("valid utf8 after strip")
}

#[test]
fn ascii_parity_with_bytes() {
    // ASCII-only; bytes and chars budgets of the same numeric value should match.
    let input =
        "[\"x\",\"x\",\"x\",\"x\",\"x\",\"x\",\"x\",\"x\",\"x\",\"x\"]";
    let out_c = run_cli(input, &["-c", "60", "-f", "json", "-t", "strict"]) // bytes
        .success();
    let out_u = run_cli(input, &["-u", "60", "-f", "json", "-t", "strict"]) // chars
        .success();
    let s_c = String::from_utf8_lossy(&out_c.get_output().stdout).into_owned();
    let s_u = String::from_utf8_lossy(&out_u.get_output().stdout).into_owned();
    assert_eq!(s_c, s_u, "ASCII output should be identical for -c and -u");
}

#[test]
fn multibyte_chars_allow_more_than_bytes_at_same_cap() {
    // Input with multi-byte characters (é). With same numeric cap, --chars can allow
    // more content than --bytes.
    let input = "[\"é\",\"é\",\"é\",\"é\",\"é\",\"é\",\"é\",\"é\",\"é\",\"é\",\"é\",\"é\"]";
    let out_bytes =
        run_cli(input, &["-c", "60", "-f", "json", "-t", "strict"]) // bytes
            .success();
    let out_chars =
        run_cli(input, &["-u", "60", "-f", "json", "-t", "strict"]) // chars
            .success();
    let s_b =
        String::from_utf8_lossy(&out_bytes.get_output().stdout).into_owned();
    let s_u =
        String::from_utf8_lossy(&out_chars.get_output().stdout).into_owned();
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
        count_fileset_headers_in_budgets: false,
        grep_highlight: None,
    };
    let cfg_color = headson::RenderConfig {
        color_enabled: true,
        ..cfg_plain.clone()
    };
    let prio = headson::PriorityConfig::new(usize::MAX, usize::MAX);

    let budgets = headson::Budgets {
        byte_budget: None,
        char_budget: Some(50),
        line_budget: None,
    };
    let grep = headson::GrepConfig::default();

    let plain = headson::headson(
        headson::InputKind::Json(input.to_vec()),
        &cfg_plain,
        &prio,
        &grep,
        budgets,
    )
    .expect("plain render under char budget");
    let colored = headson::headson(
        headson::InputKind::Json(input.to_vec()),
        &cfg_color,
        &prio,
        &grep,
        budgets,
    )
    .expect("colored render under char budget");

    // Ensure char budget enforced on uncolored output
    assert!(plain.chars().count() <= budgets.char_budget.unwrap());
    // Stripping ANSI from colored should match plain logical content
    let colored_stripped = strip_ansi(&colored);
    assert_eq!(plain, colored_stripped);
}

#[test]
fn combined_chars_and_lines_caps_enforced() {
    let p = "tests/fixtures/explicit/object_small.json";
    let content = std::fs::read_to_string(p).expect("read fixture");
    // Case A: line cap is binding
    let out_a = run_cli(
        &content,
        &["-f", "json", "-t", "default", "-n", "2", "-u", "100000"],
    ) // huge char cap
    .success();
    let s_a = String::from_utf8_lossy(&out_a.get_output().stdout).into_owned();
    assert!(
        count_lines_normalized(&s_a) <= 2,
        "line cap failed: {s_a:?}"
    );

    // Case B: char cap is binding
    let out_b = run_cli(
        &content,
        &["-f", "json", "-t", "default", "-n", "100", "-u", "60"],
    ) // small char cap
    .success();
    let s_b = String::from_utf8_lossy(&out_b.get_output().stdout).into_owned();
    assert!(
        count_chars_normalized(&s_b) <= 60,
        "char cap failed: {s_b:?}"
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

    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("hson");
    let assert = cmd
        .args([
            "--no-color",
            "-H",
            "-u",
            "40",
            "-f",
            "auto",
            a.to_str().unwrap(),
            b.to_str().unwrap(),
        ])
        .assert()
        .success();
    let out =
        String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    // Total char count should be <= per-file cap * number_of_inputs
    assert!(
        count_chars_normalized(&out) <= 80,
        "fileset char budget not enforced: {} > 80\n{}",
        count_chars_normalized(&out),
        out
    );
}
