use super::{ArrayCtx, ObjectCtx};
use crate::serialization::output::Out;
use crate::serialization::templates::core::{
    push_array_items_with, push_object_items,
};

pub(super) fn render_array(ctx: &ArrayCtx, out: &mut Out<'_>) {
    if ctx.children_len == 0 {
        if !ctx.inline_open {
            out.push_indent(ctx.depth);
        }
        out.push_str("[]");
        return;
    }
    if !ctx.inline_open {
        out.push_indent(ctx.depth);
    }
    out.push_char('[');
    out.push_newline();
    // JSON has no explicit omitted markers; just items and close.
    push_array_items_with::<crate::serialization::templates::core::StyleNoop>(
        out, ctx,
    );
    out.push_indent(ctx.depth);
    out.push_char(']');
}

pub(super) fn render_object(ctx: &ObjectCtx<'_>, out: &mut Out<'_>) {
    if ctx.children_len == 0 {
        if !ctx.inline_open {
            out.push_indent(ctx.depth);
        }
        out.push_str("{}");
        return;
    }
    if !ctx.inline_open {
        out.push_indent(ctx.depth);
    }
    out.push_char('{');
    out.push_newline();
    push_object_items(out, ctx);
    out.push_indent(ctx.depth);
    out.push_char('}');
}
