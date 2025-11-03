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
    if ctx.omitted_at_start && ctx.omitted > 0 {
        push_text_omission_line(out, indent_depth, ctx.omitted);
    }
    for (_, (_, item)) in ctx.children.iter() {
        // Remove a single trailing newline to avoid introducing blank lines
        // when we re-add newlines after indenting.
        let trimmed = item.strip_suffix('\n').unwrap_or(item);
        for line in trimmed.split('\n') {
            if indent_depth > 0 {
                out.push_indent(indent_depth);
            }
            out.push_str(line);
            out.push_newline();
        }
    }
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
