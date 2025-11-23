fn strip_ansi(s: &str) -> String {
    // Minimal SGR stripper for tests: remove ESC [ ... m sequences, preserve UTF-8.
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
            // Skip until an 'm' or end
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
fn colored_and_plain_outputs_should_match_after_stripping() {
    // Arrange a small array whose render sits near the byte budget edge.
    // Coloring adds ANSI SGR sequences to strings, which do not count toward
    // the budget: measuring is done on uncolored output, so inclusion is
    // identical after stripping colors.
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
    };
    let cfg_color = headson::RenderConfig {
        color_enabled: true,
        ..cfg_plain.clone()
    };
    let prio = headson::PriorityConfig::new(usize::MAX, usize::MAX);

    // Use a tight budget so the number of kept items is sensitive to extra bytes.
    let budget = 50usize;

    let budgets = headson::Budgets {
        byte_budget: Some(budget),
        char_budget: None,
        line_budget: None,
    };

    let plain = headson::headson(input.to_vec(), &cfg_plain, &prio, budgets)
        .expect("plain render");
    let colored = headson::headson(input.to_vec(), &cfg_color, &prio, budgets)
        .expect("color render");

    let colored_stripped = strip_ansi(&colored);

    // Expect identical logical output after stripping ANSI.
    assert_eq!(plain, colored_stripped);
}
