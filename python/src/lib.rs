use anyhow::{bail, Result};
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::PyModule;
use headson_core::{OutputTemplate, RenderConfig, PriorityConfig};

fn to_template(s: &str) -> Result<OutputTemplate> {
    match s.to_ascii_lowercase().as_str() {
        "json" => Ok(OutputTemplate::Json),
        "pseudo" | "ps" => Ok(OutputTemplate::Pseudo),
        "js" | "javascript" => Ok(OutputTemplate::Js),
        _ => bail!("unknown template: {} (expected 'json' | 'pseudo' | 'js')", s),
    }
}

fn render_config(template: &str, prefer_tail_arrays: bool) -> Result<RenderConfig> {
    let t = to_template(template)?;
    let space = " ".to_string();
    let newline = "\n".to_string();
    let indent_unit = "  ".to_string();
    Ok(RenderConfig {
        template: t,
        indent_unit,
        space,
        newline,
        prefer_tail_arrays,
    })
}

fn priority_config(per_file_budget: usize, prefer_tail_arrays: bool) -> PriorityConfig {
    PriorityConfig {
        max_string_graphemes: 500,
        array_max_items: (per_file_budget / 2).max(1),
        prefer_tail_arrays,
        array_bias: headson_core::ArrayBias::HeadMidTail,
        array_sampler: headson_core::ArraySamplerStrategy::Default,
    }
}

fn to_pyerr(e: anyhow::Error) -> PyErr {
    PyRuntimeError::new_err(format!("{}", e))
}

#[pyfunction]
#[pyo3(signature = (text, *, template="pseudo", character_budget=None, tail=false))]
fn summarize(
    py: Python<'_>,
    text: &str,
    template: &str,
    character_budget: Option<usize>,
    tail: bool,
) -> PyResult<String> {
    let cfg = render_config(template, tail).map_err(to_pyerr)?;
    let budget = character_budget.unwrap_or(500);
    let per_file_for_priority = budget.max(1);
    let prio = priority_config(per_file_for_priority, tail);
    let input = text.as_bytes().to_vec();
    py.detach(|| headson_core::headson(input, &cfg, &prio, budget).map_err(to_pyerr))
}

#[pymodule]
fn headson(_py: Python, m: &Bound<PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(summarize, m)?)?;
    Ok(())
}
