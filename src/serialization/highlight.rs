use std::path::Path;

use once_cell::sync::Lazy;
use syntect::{
    easy::HighlightLines,
    highlighting::{Style, Theme, ThemeSet},
    parsing::{SyntaxReference, SyntaxSet},
    util::as_24_bit_terminal_escaped,
};

// Load the built-in syntax/theme sets once per process.
static SYNTAXES: Lazy<SyntaxSet> =
    Lazy::new(SyntaxSet::load_defaults_newlines);
static THEME: Lazy<Theme> = Lazy::new(|| {
    let themes = ThemeSet::load_defaults();
    themes
        .themes
        .get("base16-ocean.dark")
        .cloned()
        .or_else(|| themes.themes.values().next().cloned())
        .unwrap_or_else(Theme::default)
});

pub struct CodeHighlighter<'a> {
    inner: HighlightLines<'a>,
}

impl CodeHighlighter<'static> {
    pub fn new(filename_hint: Option<&str>) -> Self {
        let syntax = syntax_for_hint(filename_hint);
        Self {
            inner: HighlightLines::new(syntax, &THEME),
        }
    }

    pub fn highlight_line(&mut self, line: &str) -> String {
        let mut owned;
        let appended_newline;
        let text = if line.ends_with('\n') {
            appended_newline = false;
            line
        } else {
            appended_newline = true;
            owned = String::with_capacity(line.len() + 1);
            owned.push_str(line);
            owned.push('\n');
            &owned
        };
        let ranges = self
            .inner
            .highlight_line(text, &SYNTAXES)
            .unwrap_or_else(|_| vec![(Style::default(), text)]);
        // Use standard 8/16 ANSI colors so user terminal themes stay in control.
        let mut s = as_24_bit_terminal_escaped(&ranges, false);
        if appended_newline {
            if let Some(pos) = s.rfind('\n') {
                s.remove(pos);
            }
        }
        s.push_str("\u{001b}[0m");
        s
    }
}

pub(crate) fn code_highlight_lines(
    order: &crate::PriorityOrder,
    array_id: usize,
    source_hint: Option<&str>,
) -> Vec<String> {
    let root = code_root_array_id(order, array_id);
    if let Some(full) = order.code_lines.get(&root) {
        let mut highlighter = CodeHighlighter::new(source_hint);
        return full
            .iter()
            .map(|line| highlighter.highlight_line(line))
            .collect();
    }
    let mut lines: Vec<Option<String>> = Vec::new();
    collect_code_lines(order, array_id, &mut lines);
    if lines.is_empty() {
        return Vec::new();
    }
    let mut highlighter = CodeHighlighter::new(source_hint);
    lines
        .into_iter()
        .map(|opt| {
            let text = opt.unwrap_or_default();
            highlighter.highlight_line(&text)
        })
        .collect()
}

pub(crate) fn collect_code_lines(
    order: &crate::PriorityOrder,
    array_id: usize,
    acc: &mut Vec<Option<String>>,
) {
    if let Some(children) = order.children.get(array_id) {
        for child in children {
            let child_idx = child.0;
            match &order.nodes[child_idx] {
                crate::RankedNode::Array { .. }
                | crate::RankedNode::Object { .. } => {
                    collect_code_lines(order, child_idx, acc);
                }
                crate::RankedNode::SplittableLeaf { value, .. } => {
                    push_code_line(order, child_idx, value, acc);
                }
                crate::RankedNode::AtomicLeaf { token, .. } => {
                    push_code_line(order, child_idx, token, acc);
                }
                crate::RankedNode::LeafPart { .. } => {}
            }
        }
    }
}

fn push_code_line(
    order: &crate::PriorityOrder,
    child_idx: usize,
    text: &str,
    acc: &mut Vec<Option<String>>,
) {
    let idx = order
        .index_in_parent_array
        .get(child_idx)
        .and_then(|o| *o)
        .unwrap_or(0);
    if acc.len() <= idx {
        acc.resize(idx + 1, None);
    }
    acc[idx] = Some(text.to_string());
}

pub(crate) fn code_root_array_id(
    order: &crate::PriorityOrder,
    array_id: usize,
) -> usize {
    let mut current = array_id;
    while let Some(Some(parent)) = order.parent.get(current) {
        match order.nodes[parent.0] {
            crate::RankedNode::Array { .. } => current = parent.0,
            _ => break,
        }
    }
    current
}

fn syntax_for_hint(hint: Option<&str>) -> &'static SyntaxReference {
    let Some(name) = hint else {
        return SYNTAXES.find_syntax_plain_text();
    };
    if let Some(syntax) = SYNTAXES.find_syntax_by_path(name) {
        return syntax;
    }
    if let Some(ext) = Path::new(name).extension().and_then(|s| s.to_str()) {
        if let Some(syntax) = SYNTAXES.find_syntax_by_extension(ext) {
            return syntax;
        }
        if let Some(alias) = syntax_alias_for_extension(ext) {
            if let Some(syntax) = SYNTAXES.find_syntax_by_name(alias) {
                return syntax;
            }
        }
    }
    SYNTAXES.find_syntax_plain_text()
}

fn syntax_alias_for_extension(ext: &str) -> Option<&'static str> {
    if ext.eq_ignore_ascii_case("ts") || ext.eq_ignore_ascii_case("tsx") {
        return Some("JavaScript");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_typescript_syntax_from_extension() {
        let syntax = syntax_for_hint(Some("example.ts"));
        assert!(
            syntax.name != "Plain Text",
            "expected non-plain syntax for .ts, got {}",
            syntax.name
        );
    }

    #[test]
    fn syntax_set_contains_typescript_extension() {
        let has_ts = SYNTAXES.syntaxes().iter().any(|syntax| {
            syntax.name.contains("JavaScript")
                && syntax
                    .file_extensions
                    .iter()
                    .any(|ext| ext.eq_ignore_ascii_case("js"))
        });
        assert!(has_ts, "SyntaxSet is missing JavaScript fallback");
    }

    #[test]
    fn detects_shell_syntax_from_extension() {
        let syntax = syntax_for_hint(Some("script.sh"));
        assert!(
            syntax.name != "Plain Text",
            "expected non-plain syntax for .sh, got {}",
            syntax.name
        );
    }

    #[test]
    fn detects_python_syntax_from_extension() {
        let syntax = syntax_for_hint(Some("file.PY"));
        assert!(
            syntax.name != "Plain Text",
            "expected non-plain syntax for .py, got {}",
            syntax.name
        );
    }

    #[test]
    fn detects_tsx_syntax_from_extension() {
        let syntax = syntax_for_hint(Some("component.tsx"));
        assert!(
            syntax.name != "Plain Text",
            "expected non-plain syntax for .tsx, got {}",
            syntax.name
        );
    }
}
