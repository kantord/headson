use super::{ArrayCtx, ObjectCtx};
use crate::serialization::output::Out;
use crate::serialization::templates::core::{
    StyleNoop, push_array_items_with, push_object_items, wrap_block,
};

pub(super) fn render_array(ctx: &ArrayCtx, out: &mut Out<'_>) {
    if ctx.children_len == 0 {
        if !ctx.inline_open {
            out.push_indent(ctx.depth);
        }
        out.push_str("[]");
        return;
    }
    wrap_block(out, ctx.depth, ctx.inline_open, '[', ']', |o| {
        // JSON has no explicit omitted markers; just items and close.
        push_array_items_with::<StyleNoop>(o, ctx);
    });
}

pub(super) fn render_object(ctx: &ObjectCtx<'_>, out: &mut Out<'_>) {
    if ctx.children_len == 0 {
        if !ctx.inline_open {
            out.push_indent(ctx.depth);
        }
        out.push_str("{}");
        return;
    }
    wrap_block(out, ctx.depth, ctx.inline_open, '{', '}', |o| {
        push_object_items(o, ctx);
    });
}
