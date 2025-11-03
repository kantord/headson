use super::{ArrayCtx, ObjectCtx};
use crate::serialization::output::Out;

fn push_text_omission_line(out: &mut Out<'_>, depth: usize, omitted: usize) {
    match out.style() {
        crate::serialization::types::Style::Strict => {}
        crate::serialization::types::Style::Default => {
            if depth > 0 {
                out.push_indent(depth);
            }
            out.push_omission();
            out.push_newline();
        }
        crate::serialization::types::Style::Detailed => {
            if depth > 0 {
                out.push_indent(depth);
            }
            out.push_omission();
            out.push_str(" ");
            out.push_str(&format!("{omitted} more lines "));
            out.push_omission();
            out.push_newline();
        }
    }
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

    // Optional omission at start (head/tail preference)
    if ctx.omitted_at_start && ctx.omitted > 0 {
        push_text_omission_line(out, indent_depth, ctx.omitted);
    }

    // Track original indices to emit omission markers for internal gaps.
    let mut prev_index: Option<usize> = None;
    for (i, (orig_index, (kind, item))) in ctx.children.iter().enumerate() {
        if let Some(prev) = prev_index {
            let gap = orig_index.saturating_sub(prev).saturating_sub(1);
            if gap > 0 {
                // Emit a gap omission marker at this nesting depth.
                push_text_omission_line(out, indent_depth, gap);
            }
        }

        // Multi-line children (e.g., nested blocks) are already indented
        // by their own renderer at depth+1; push as-is to avoid double indent.
        let is_multiline = item.contains('\n');
        match kind {
            super::super::NodeKind::Array | super::super::NodeKind::Object => {
                out.push_str(item);
            }
            _ if is_multiline => {
                out.push_str(item);
            }
            _ => {
                // Leaf line: indent according to this array's depth.
                if indent_depth > 0 {
                    out.push_indent(indent_depth);
                }
                out.push_str(item);
                out.push_newline();
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
        push_text_omission_line(out, indent_depth, ctx.omitted);
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
