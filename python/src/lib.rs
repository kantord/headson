use anyhow::{bail, Result};
use headson_core::{
    ArraySamplerStrategy, Budgets, ColorMode, GrepConfig, InputKind,
    OutputTemplate, PriorityConfig, RenderConfig, Style,
};
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::PyModule;
use regex::RegexBuilder;

fn to_style(s: &str) -> Result<Style> {
    match s.to_ascii_lowercase().as_str() {
        "strict" => Ok(Style::Strict),
        "default" => Ok(Style::Default),
        "detailed" => Ok(Style::Detailed),
        other => bail!(
            "unknown style: {} (expected 'strict' | 'default' | 'detailed')",
            other
        ),
    }
}

fn map_json_template_for_style(style: Style) -> OutputTemplate {
    match style {
        Style::Strict => OutputTemplate::Json,
        Style::Default => OutputTemplate::Pseudo,
        Style::Detailed => OutputTemplate::Js,
    }
}

fn map_output_template(format: &str, style: Style) -> Result<OutputTemplate> {
    match format.to_ascii_lowercase().as_str() {
        "auto" => Ok(map_json_template_for_style(style)), // stdin => JSON family
        "json" => Ok(map_json_template_for_style(style)),
        "yaml" | "yml" => Ok(OutputTemplate::Yaml),
        "text" => Ok(OutputTemplate::Text),
        "code" => Ok(OutputTemplate::Code),
        other => bail!(
            "unknown format: {} (expected 'auto' | 'json' | 'yaml' | 'text' | 'code')",
            other
        ),
    }
}

fn render_config_with_sampler(
    format: &str,
    style: &str,
    sampler: ArraySamplerStrategy,
) -> Result<RenderConfig> {
    let s = to_style(style)?;
    let t = map_output_template(format, s)?;
    let space = " ".to_string();
    let newline = "\n".to_string();
    let indent_unit = "  ".to_string();
    let prefer_tail_arrays = matches!(sampler, ArraySamplerStrategy::Tail);
    let color_mode = ColorMode::Off;
    let color_enabled = false;
    Ok(RenderConfig {
        template: t,
        indent_unit,
        space,
        newline,
        prefer_tail_arrays,
        color_mode,
        color_enabled,
        style: s,
        string_free_prefix_graphemes: None,
        debug: false,
        primary_source_name: None,
        show_fileset_headers: true,
        count_fileset_headers_in_budgets: false,
        grep_highlight: None,
    })
}

fn parse_skew(skew: &str) -> Result<ArraySamplerStrategy> {
    match skew.to_ascii_lowercase().as_str() {
        "balanced" => Ok(ArraySamplerStrategy::Default),
        "head" => Ok(ArraySamplerStrategy::Head),
        "tail" => Ok(ArraySamplerStrategy::Tail),
        other => bail!(
            "unknown skew: {} (expected 'balanced' | 'head' | 'tail')",
            other
        ),
    }
}

fn priority_config(
    per_file_budget: usize,
    sampler: ArraySamplerStrategy,
) -> PriorityConfig {
    let prefer_tail_arrays = matches!(sampler, ArraySamplerStrategy::Tail);
    PriorityConfig {
        max_string_graphemes: 500,
        array_max_items: (per_file_budget / 2).max(1),
        prefer_tail_arrays,
        array_bias: headson_core::ArrayBias::HeadMidTail,
        array_sampler: sampler,
        line_budget_only: false,
    }
}

fn to_pyerr(e: anyhow::Error) -> PyErr {
    PyRuntimeError::new_err(format!("{}", e))
}

#[pyfunction]
#[allow(clippy::too_many_arguments)] // Python API surface requires these knobs
#[pyo3(signature = (text, *, format="auto", style="default", byte_budget=None, skew="balanced", input_format="json", grep=None))]
fn summarize(
    py: Python<'_>,
    text: &str,
    format: &str,
    style: &str,
    byte_budget: Option<usize>,
    skew: &str,
    input_format: &str,
    grep: Option<&str>,
) -> PyResult<String> {
    let grep_re = if let Some(pat) = grep {
        Some(
            RegexBuilder::new(pat)
                .unicode(true)
                .build()
                .map_err(|e| to_pyerr(e.into()))?,
        )
    } else {
        None
    };
    let sampler = parse_skew(skew).map_err(to_pyerr)?;
    let mut cfg = render_config_with_sampler(format, style, sampler)
        .map_err(to_pyerr)?;
    let budget = byte_budget.unwrap_or(500);
    let per_file_for_priority = budget.max(1);
    let prio = priority_config(per_file_for_priority, sampler);
    let input = text.as_bytes().to_vec();
    if let Some(re) = &grep_re {
        cfg.grep_highlight = Some(re.clone());
    }
    let grep_cfg = GrepConfig {
        regex: grep_re,
        weak: false,
    };
    let budgets = Budgets {
        byte_budget: Some(budget),
        char_budget: None,
        line_budget: None,
    };
    let text_mode = if matches!(cfg.template, OutputTemplate::Code) {
        headson_core::TextMode::CodeLike
    } else {
        headson_core::TextMode::Plain
    };
    py.detach(|| match input_format.to_ascii_lowercase().as_str() {
        "json" => headson_core::headson(
            InputKind::Json(input),
            &cfg,
            &prio,
            &grep_cfg,
            budgets,
        )
        .map_err(to_pyerr),
        "yaml" | "yml" => headson_core::headson(
            InputKind::Yaml(input),
            &cfg,
            &prio,
            &grep_cfg,
            budgets,
        )
        .map_err(to_pyerr),
        "text" => headson_core::headson(
            InputKind::Text {
                bytes: input,
                mode: text_mode,
            },
            &cfg,
            &prio,
            &grep_cfg,
            budgets,
        )
        .map_err(to_pyerr),
        other => Err(to_pyerr(anyhow::anyhow!(
            "unknown input_format: {} (expected 'json' | 'yaml' | 'text')",
            other
        ))),
    })
}

#[pymodule]
fn headson(_py: Python, m: &Bound<PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(summarize, m)?)?;
    Ok(())
}
