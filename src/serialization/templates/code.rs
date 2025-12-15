use super::{ArrayCtx, ObjectCtx};
use crate::serialization::highlight::CodeHighlighter;
use crate::serialization::output::Out;

// Compute the leading whitespace (spaces/tabs) prefix of a single line.
fn leading_ws_prefix(s: &str) -> &str {
    let mut end = 0usize;
    for (i, b) in s.as_bytes().iter().enumerate() {
        match *b {
            b' ' | b'\t' => end = i + 1,
            _ => break,
        }
    }
    &s[..end]
}

fn last_nonempty_line_indent(s: &str) -> Option<&str> {
    for line in s.rsplit('\n') {
        if !line.trim().is_empty() {
            return Some(leading_ws_prefix(line));
        }
    }
    None
}

// No explicit omission markers for the code template: jumps in the printed
// line numbers are the omission signal (e.g., `4:` → `22:` means 17 lines were
// skipped).

pub(super) fn render_array(ctx: &ArrayCtx<'_>, out: &mut Out<'_>) {
    let mut last_nonempty_indent: String = String::new();
    let highlight_lookup = ctx.code_highlight.as_deref();
    let mut fallback_highlighter =
        if highlight_lookup.is_none() && out.colors_enabled() {
            Some(CodeHighlighter::new(ctx.source_hint))
        } else {
            None
        };

    for (orig_index, (kind, item)) in ctx.children.iter() {
        if matches!(
            kind,
            super::super::NodeKind::Array | super::super::NodeKind::Object
        ) || item.contains('\n')
        {
            render_block(item, &mut last_nonempty_indent, out);
            continue;
        }
        render_leaf_line(
            *orig_index,
            item,
            &mut last_nonempty_indent,
            highlight_lookup,
            fallback_highlighter.as_mut(),
            out,
        );
    }
}

pub(super) fn render_object(ctx: &ObjectCtx<'_>, out: &mut Out<'_>) {
    // Code template defines custom rendering only for arrays (raw lines).
    super::pseudo::render_object(ctx, out);
}

fn render_block(
    item: &str,
    last_nonempty_indent: &mut String,
    out: &mut Out<'_>,
) {
    out.push_str(item);
    update_last_indent(item, last_nonempty_indent);
}

fn render_leaf_line(
    orig_index: usize,
    item: &str,
    last_nonempty_indent: &mut String,
    highlight_lookup: Option<&Vec<String>>,
    mut fallback_highlighter: Option<&mut CodeHighlighter<'static>>,
    out: &mut Out<'_>,
) {
    let n = orig_index.saturating_add(1);
    if let Some(w) = out.line_number_width() {
        out.push_str(&format!("{n:>w$}: "));
    } else {
        out.push_str(&format!("{n}: "));
    }
    match highlight_lookup.and_then(|lines| lines.get(orig_index)) {
        Some(colored) => out.push_str(colored),
        None => {
            if let Some(hl) = fallback_highlighter.as_mut() {
                let colored = hl.highlight_line(item);
                out.push_str(&colored);
            } else {
                out.push_str(item);
            }
        }
    }
    out.push_newline();
    if !item.trim().is_empty() {
        last_nonempty_indent.clear();
        last_nonempty_indent.push_str(leading_ws_prefix(item));
    }
}

fn update_last_indent(item: &str, last_nonempty_indent: &mut String) {
    if let Some(ind) = last_nonempty_line_indent(item) {
        if !ind.is_empty() {
            last_nonempty_indent.clear();
            last_nonempty_indent.push_str(ind);
        }
    }
}
