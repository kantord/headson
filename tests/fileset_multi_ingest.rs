use headson::{
    ArrayBias, Budgets, ColorMode, FilesetInput, FilesetInputKind,
    OutputTemplate, PriorityConfig, RenderConfig, Style,
};
use insta::assert_snapshot;

fn render_config() -> RenderConfig {
    RenderConfig {
        template: OutputTemplate::Auto,
        indent_unit: "  ".to_string(),
        space: " ".to_string(),
        newline: "\n".to_string(),
        prefer_tail_arrays: false,
        color_mode: ColorMode::Off,
        color_enabled: false,
        style: Style::Default,
        string_free_prefix_graphemes: None,
        debug: false,
        primary_source_name: None,
        show_fileset_headers: true,
        count_fileset_headers_in_budgets: false,
    }
}

fn priority_config() -> PriorityConfig {
    let mut cfg = PriorityConfig::new(256, 2);
    cfg.array_bias = ArrayBias::Head;
    cfg
}

#[test]
fn fileset_multi_format_snapshot() {
    let inputs = vec![
        FilesetInput {
            name: "config.json".into(),
            bytes: br#"{
  "service": {
    "enabled": true,
    "endpoints": [
      "/health",
      "/status",
      "/live",
      "/ready",
      "/metrics"
    ]
  }
}"#
            .to_vec(),
            kind: FilesetInputKind::Json,
        },
        FilesetInput {
            name: "settings.yaml".into(),
            bytes: r#"
environments:
  prod:
    replicas: 4
    region: eu-west-1
  staging:
    replicas: 1
    region: us-east-1
"#
            .trim()
            .as_bytes()
            .to_vec(),
            kind: FilesetInputKind::Yaml,
        },
        FilesetInput {
            name: "script.sh".into(),
            bytes:
                b"#!/usr/bin/env bash\nset -euo pipefail\necho \"deploy\"\n"
                    .to_vec(),
            kind: FilesetInputKind::Text { atomic_lines: true },
        },
    ];

    let grep = headson::GrepConfig::default();
    let out = headson::headson(
        headson::InputKind::Fileset(inputs),
        &render_config(),
        &priority_config(),
        &grep,
        Budgets {
            byte_budget: Some(4096),
            char_budget: None,
            line_budget: None,
        },
    )
    .expect("render fileset");

    assert_snapshot!("fileset_multi_format_snapshot", out);
}
