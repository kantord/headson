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

fn push_text_omission_line(
    out: &mut Out<'_>,
    depth: usize,
    omitted: usize,
    indent_prefix: Option<&str>,
) {
    use crate::serialization::types::Style;
    let style = out.style();
    if matches!(style, Style::Strict) {
        return;
    }
    push_optional_indent(out, depth, indent_prefix);
    out.push_omission();
    if matches!(style, Style::Detailed) {
        out.push_str(" ");
        out.push_str(&format!("{omitted} more lines "));
        out.push_omission();
    }
    out.push_newline();
}

#[allow(
    clippy::cognitive_complexity,
    reason = "Indent + omission flow is clearer inline"
)]
pub(super) fn render_array(ctx: &ArrayCtx, out: &mut Out<'_>) {
    // For text, arrays are treated as raw lines of text.
    // Indentation depth for lines equals (ctx.depth - 1), so top-level
    // line-arrays render without indentation, and nested arrays increase it.
    let indent_depth = ctx.depth.saturating_sub(1);

    // Track the last seen non-empty line's textual indent. This lets us
    // indent omission markers to match surrounding context even for
    // top-level text arrays (e.g., code files), without parsing semantics.
    let mut last_nonempty_indent: String = String::new();

    // If we start with an omission, try to peek the first child's textual indent
    // so we can align the starting omission marker with the upcoming context.
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
        push_text_omission_line(
            out,
            indent_depth,
            ctx.omitted,
            start_peek_indent.as_deref(),
        );
    }

    // Track original indices to emit omission markers for internal gaps.
    let mut prev_index: Option<usize> = None;
    let mut saw_header_line = false;
    for (i, (orig_index, (kind, item))) in ctx.children.iter().enumerate() {
        if let Some(prev) = prev_index {
            let gap = orig_index.saturating_sub(prev).saturating_sub(1);
            if gap > 0 {
                // Emit a gap omission marker aligned with the last non-empty
                // line's textual indent when available; otherwise structural.
                let prefix_opt = if !last_nonempty_indent.is_empty() {
                    Some(last_nonempty_indent.as_str())
                } else {
                    // Try to align with the upcoming line's indent as a best-effort.
                    let next_pref = leading_ws_prefix(item);
                    if next_pref.is_empty() {
                        None
                    } else {
                        Some(next_pref)
                    }
                };
                push_text_omission_line(out, indent_depth, gap, prefix_opt);
            }
        }

        // Multi-line children (e.g., nested blocks) are already indented
        // by their own renderer at depth+1; push as-is to avoid double indent.
        let is_multiline = item.contains('\n');
        match kind {
            super::super::NodeKind::Array | super::super::NodeKind::Object => {
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
                // Leaf line: print exactly as in source (keeps original indent).
                out.push_str(item);
                out.push_newline();
                // Update last non-empty indent based on this line.
                if !item.trim().is_empty() {
                    last_nonempty_indent.clear();
                    last_nonempty_indent.push_str(leading_ws_prefix(item));
                }
                // Mark that this array printed its header/source line; subsequent
                // omissions belong to its nested block visually.
                if !saw_header_line {
                    saw_header_line = true;
                }
            }
        }

        // Remember last original index we printed.
        prev_index = Some(*orig_index);
        // Ensure we don't accidentally miss a trailing comma/newline logic:
        // text template prints one line per child; nested children include
        // their own trailing newlines in their rendered form.
        let _ = i; // silence unused in case of cfg differences
    }

    // Optional omission at end (head/tail preference)
    if !ctx.omitted_at_start && ctx.omitted > 0 {
        let prefix_opt = if last_nonempty_indent.is_empty() {
            None
        } else {
            Some(last_nonempty_indent.as_str())
        };
        // Only apply textual indentation; do not fall back to structural.
        push_text_omission_line(out, indent_depth, ctx.omitted, prefix_opt);
    }
}

pub(super) fn render_object(ctx: &ObjectCtx<'_>, out: &mut Out<'_>) {
    // Text template defines custom rendering only for arrays (raw lines).
    // Objects should not normally appear under the text template because
    // fileset roots are handled by the dedicated fileset renderer before
    // template dispatch. If an object does reach here (defensive case),
    // delegate to the generic pseudo object renderer for a consistent shape.
    super::pseudo::render_object(ctx, out);
}
