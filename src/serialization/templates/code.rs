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

#[allow(
    clippy::cognitive_complexity,
    reason = "Indent + omission flow is clearer inline"
)]
pub(super) fn render_array(ctx: &ArrayCtx<'_>, out: &mut Out<'_>) {
    // For code, arrays are treated as raw lines of text with line numbers.
    let _indent_depth = ctx.depth.saturating_sub(1);

    // Track the last seen non-empty line's textual indent for potential future alignment.
    let mut last_nonempty_indent: String = String::new();
    let highlight_lookup = ctx.code_highlight.as_ref();
    let mut fallback_highlighter =
        if highlight_lookup.is_none() && out.colors_enabled() {
            Some(CodeHighlighter::new(ctx.source_hint))
        } else {
            None
        };

    // No omission marker at start for code template.

    for (orig_index, (kind, item)) in ctx.children.iter() {
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
                let n = orig_index.saturating_add(1);
                if let Some(w) = out.line_number_width() {
                    out.push_str(&format!("{n:>w$}: "));
                } else {
                    out.push_str(&format!("{n}: "));
                }
                match highlight_lookup.and_then(|lines| lines.get(*orig_index))
                {
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
        }
    }

    // No omission marker at end for code template.
}

pub(super) fn render_object(ctx: &ObjectCtx<'_>, out: &mut Out<'_>) {
    // Code template defines custom rendering only for arrays (raw lines).
    super::pseudo::render_object(ctx, out);
}
