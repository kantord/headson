use super::{ArrayCtx, ObjectCtx};
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

#[inline]
fn push_optional_indent(
    out: &mut Out<'_>,
    depth: usize,
    indent_prefix: Option<&str>,
) {
    if let Some(p) = indent_prefix.filter(|p| !p.is_empty()) {
        out.push_str(p);
    } else if depth > 0 {
        // Fall back to structural indentation only for omission markers.
        out.push_indent(depth);
    }
}

fn push_code_omission_line(
    out: &mut Out<'_>,
    depth: usize,
    _omitted: usize,
    indent_prefix: Option<&str>,
) {
    // In code template, omissions render as a single ellipsis line regardless of style.
    if matches!(out.style(), crate::serialization::types::Style::Strict) {
        return;
    }
    push_optional_indent(out, depth, indent_prefix);
    out.push_omission();
    out.push_newline();
}

#[allow(
    clippy::cognitive_complexity,
    reason = "Indent + omission flow is clearer inline"
)]
pub(super) fn render_array(ctx: &ArrayCtx, out: &mut Out<'_>) {
    // For code, arrays are treated as raw lines of text with line numbers.
    let indent_depth = ctx.depth.saturating_sub(1);

    // Track the last seen non-empty line's textual indent for omission alignment.
    let mut last_nonempty_indent: String = String::new();

    // If we start with an omission, try to peek the first child's textual indent.
    let start_peek_indent: Option<String> =
        ctx.children
            .first()
            .and_then(|(_idx, (kind, item))| match kind {
                super::super::NodeKind::Array
                | super::super::NodeKind::Object => None,
                _ => {
                    let p = leading_ws_prefix(item);
                    if item.trim().is_empty() {
                        None
                    } else {
                        Some(p.to_string())
                    }
                }
            });

    // Optional omission at start (head/tail preference)
    if ctx.omitted_at_start && ctx.omitted > 0 {
        push_code_omission_line(
            out,
            indent_depth,
            ctx.omitted,
            start_peek_indent.as_deref(),
        );
    }

    // Track original indices to emit omission markers for internal gaps.
    let mut prev_index: Option<usize> = None;
    for (_i, (orig_index, (kind, item))) in ctx.children.iter().enumerate() {
        if let Some(prev) = prev_index {
            let gap = orig_index.saturating_sub(prev).saturating_sub(1);
            if gap > 0 {
                // Emit a gap omission marker aligned to surrounding context.
                let prefix_opt = if !last_nonempty_indent.is_empty() {
                    Some(last_nonempty_indent.as_str())
                } else {
                    let next_pref = leading_ws_prefix(item);
                    if next_pref.is_empty() {
                        None
                    } else {
                        Some(next_pref)
                    }
                };
                push_code_omission_line(out, indent_depth, gap, prefix_opt);
            }
        }

        let is_multiline = item.contains('\n');
        match kind {
            super::super::NodeKind::Array | super::super::NodeKind::Object => {
                // Nested blocks are rendered verbatim.
                out.push_str(item);
                if let Some(ind) = last_nonempty_line_indent(item) {
                    if !ind.is_empty() {
                        last_nonempty_indent.clear();
                        last_nonempty_indent.push_str(ind);
                    }
                }
            }
            _ if is_multiline => {
                out.push_str(item);
                if let Some(ind) = last_nonempty_line_indent(item) {
                    if !ind.is_empty() {
                        last_nonempty_indent.clear();
                        last_nonempty_indent.push_str(ind);
                    }
                }
            }
            _ => {
                // Leaf line: print line number and content.
                if out.line_numbers_enabled() {
                    let n = orig_index.saturating_add(1);
                    if let Some(w) = out.line_number_width() {
                        out.push_str(&format!("{:>width$}: ", n, width = w));
                    } else {
                        out.push_str(&format!("{n}: "));
                    }
                }
                out.push_str(item);
                out.push_newline();
                if !item.trim().is_empty() {
                    last_nonempty_indent.clear();
                    last_nonempty_indent.push_str(leading_ws_prefix(item));
                }
            }
        }

        prev_index = Some(*orig_index);
    }

    // Optional omission at end (head/tail preference)
    if !ctx.omitted_at_start && ctx.omitted > 0 {
        let prefix_opt = if last_nonempty_indent.is_empty() {
            None
        } else {
            Some(last_nonempty_indent.as_str())
        };
        push_code_omission_line(out, indent_depth, ctx.omitted, prefix_opt);
    }
}

pub(super) fn render_object(ctx: &ObjectCtx<'_>, out: &mut Out<'_>) {
    // Code template defines custom rendering only for arrays (raw lines).
    super::pseudo::render_object(ctx, out);
}
