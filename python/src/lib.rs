use anyhow::{bail, Context, Result};
use pyo3::exceptions::{PyRuntimeError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyBytes, PyDict, PyList};

fn to_template(s: &str) -> Result<headson::OutputTemplate> {
    match s.to_ascii_lowercase().as_str() {
        "json" => Ok(headson::OutputTemplate::Json),
        "pseudo" | "ps" => Ok(headson::OutputTemplate::Pseudo),
        "js" | "javascript" => Ok(headson::OutputTemplate::Js),
        _ => bail!("unknown template: {} (expected 'json' | 'pseudo' | 'js')", s),
    }
}

fn render_config(
    template: &str,
    compact: bool,
    indent: Option<&str>,
    no_space: bool,
    no_newline: bool,
) -> Result<headson::RenderConfig> {
    let t = to_template(template)?;
    let space = if compact || no_space { "" } else { " " }.to_string();
    let newline = if compact || no_newline { "" } else { "\n" }.to_string();
    let indent_unit = if compact {
        String::new()
    } else {
        indent.unwrap_or("  ").to_string()
    };
    Ok(headson::RenderConfig {
        template: t,
        indent_unit,
        space,
        newline,
    })
}

fn priority_config(per_file_budget: usize, string_cap: usize) -> headson::PriorityConfig {
    headson::PriorityConfig {
        max_string_graphemes: string_cap,
        array_max_items: (per_file_budget / 2).max(1),
    }
}

fn to_pyerr(e: anyhow::Error) -> PyErr {
    PyRuntimeError::new_err(format!("{}", e))
}

#[pyfunction]
#[pyo3(signature = (data, *, template="pseudo", budget=None, global_budget=None, compact=false, indent="  ", no_space=false, no_newline=false, string_cap=500))]
fn summarize_bytes(
    py: Python<'_>,
    data: &PyBytes,
    template: &str,
    budget: Option<usize>,
    global_budget: Option<usize>,
    compact: bool,
    indent: &str,
    no_space: bool,
    no_newline: bool,
    string_cap: usize,
) -> PyResult<String> {
    let cfg = render_config(template, compact, Some(indent), no_space, no_newline)
        .map_err(to_pyerr)?;
    let per_file = budget.unwrap_or(500);
    let effective_budget = global_budget.unwrap_or(per_file);
    let per_file_for_priority = effective_budget.max(1);
    let prio = priority_config(per_file_for_priority, string_cap);
    let input = data.as_bytes().to_vec();
    py.allow_threads(|| headson::headson(input, &cfg, &prio, effective_budget).map_err(to_pyerr))
}

#[pyfunction]
#[pyo3(signature = (paths, *, template="pseudo", budget=None, global_budget=None, compact=false, indent="  ", no_space=false, no_newline=false, string_cap=500))]
fn summarize_files(
    py: Python<'_>,
    paths: &PyList,
    template: &str,
    budget: Option<usize>,
    global_budget: Option<usize>,
    compact: bool,
    indent: &str,
    no_space: bool,
    no_newline: bool,
    string_cap: usize,
) -> PyResult<String> {
    if paths.is_empty() {
        return Err(PyValueError::new_err("paths list must not be empty"));
    }
    let cfg = render_config(template, compact, Some(indent), no_space, no_newline)
        .map_err(to_pyerr)?;
    let per_file = budget.unwrap_or(500);
    let input_count = paths.len();
    let effective_budget = if let Some(g) = global_budget {
        g
    } else {
        per_file.saturating_mul(input_count)
    };
    let per_file_for_priority = (effective_budget / input_count).max(1);
    let prio = priority_config(per_file_for_priority, string_cap);

    // Collect file contents
    let mut inputs: Vec<(String, Vec<u8>)> = Vec::with_capacity(input_count);
    for item in paths.iter() {
        let path_str: String = if let Ok(s) = item.extract() {
            s
        } else if let Ok(p) = item.call_method0("__fspath__") {
            p.extract().map_err(|_| PyTypeError::new_err("path must be str or os.PathLike"))?
        } else {
            return Err(PyTypeError::new_err("path must be str or os.PathLike"));
        };
        let bytes = std::fs::read(&path_str).with_context(|| format!("failed to read file: {}", path_str)).map_err(to_pyerr)?;
        inputs.push((path_str, bytes));
    }

    py.allow_threads(|| headson::headson_many(inputs, &cfg, &prio, effective_budget).map_err(to_pyerr))
}

#[pyfunction]
#[pyo3(signature = (items, *, template="pseudo", budget=None, global_budget=None, compact=false, indent="  ", no_space=false, no_newline=false, string_cap=500))]
fn summarize_texts(
    py: Python<'_>,
    items: &PyList,
    template: &str,
    budget: Option<usize>,
    global_budget: Option<usize>,
    compact: bool,
    indent: &str,
    no_space: bool,
    no_newline: bool,
    string_cap: usize,
) -> PyResult<String> {
    if items.is_empty() {
        return Err(PyValueError::new_err("items list must not be empty"));
    }
    let cfg = render_config(template, compact, Some(indent), no_space, no_newline)
        .map_err(to_pyerr)?;
    let per_file = budget.unwrap_or(500);
    let input_count = items.len();
    let effective_budget = if let Some(g) = global_budget { g } else { per_file.saturating_mul(input_count) };
    let per_file_for_priority = (effective_budget / input_count).max(1);
    let prio = priority_config(per_file_for_priority, string_cap);

    let mut inputs: Vec<(String, Vec<u8>)> = Vec::with_capacity(input_count);
    for item in items.iter() {
        if let Ok(d) = item.downcast::<PyDict>() {
            let path: String = d
                .get_item("path")
                .ok_or_else(|| PyValueError::new_err("item missing 'path'"))?
                .extract()
                .map_err(|_| PyTypeError::new_err("'path' must be str"))?;
            // content may be str or bytes
            let content_obj = d
                .get_item("content")
                .ok_or_else(|| PyValueError::new_err("item missing 'content'"))?;
            let bytes: Vec<u8> = if let Ok(s) = content_obj.extract::<&str>() {
                s.as_bytes().to_vec()
            } else if let Ok(b) = content_obj.downcast::<PyBytes>() {
                b.as_bytes().to_vec()
            } else {
                return Err(PyTypeError::new_err("'content' must be str or bytes"));
            };
            inputs.push((path, bytes));
        } else {
            return Err(PyTypeError::new_err("items must be list of dicts with 'path' and 'content'"));
        }
    }

    py.allow_threads(|| headson::headson_many(inputs, &cfg, &prio, effective_budget).map_err(to_pyerr))
}

#[pymodule]
fn headson(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(summarize_bytes, m)?)?;
    m.add_function(wrap_pyfunction!(summarize_files, m)?)?;
    m.add_function(wrap_pyfunction!(summarize_texts, m)?)?;
    Ok(())
}

