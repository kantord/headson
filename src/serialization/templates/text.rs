use super::{ArrayCtx, ObjectCtx};
use crate::serialization::output::Out;

fn push_text_omission_line(out: &mut Out<'_>, omitted: usize) {
    match out.style() {
        crate::serialization::types::Style::Strict => {}
        crate::serialization::types::Style::Default => {
            out.push_omission();
            out.push_newline();
        }
        crate::serialization::types::Style::Detailed => {
            out.push_omission();
            out.push_str(" ");
            out.push_str(&format!("{omitted} more lines "));
            out.push_omission();
            out.push_newline();
        }
    }
}

pub(super) fn render_array(ctx: &ArrayCtx, out: &mut Out<'_>) {
    // For text, arrays are treated as raw lines of text. We do not emit
    // brackets or indentation; we only write lines and optional omission markers.
    if ctx.omitted_at_start && ctx.omitted > 0 {
        push_text_omission_line(out, ctx.omitted);
    }
    for (_, (_, item)) in ctx.children.iter() {
        out.push_str(item);
        out.push_newline();
    }
    if !ctx.omitted_at_start && ctx.omitted > 0 {
        push_text_omission_line(out, ctx.omitted);
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
