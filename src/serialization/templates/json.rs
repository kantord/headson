use super::core::{Style, render_array_with, render_object_with};
use super::{ArrayCtx, ObjectCtx};
use crate::serialization::output::Out;

struct Json;

impl Style for Json {
    fn array_empty(out: &mut Out<'_>, ctx: &ArrayCtx) {
        if !ctx.inline_open {
            out.push_indent(ctx.depth);
        }
        out.push_str("[]");
    }

    fn object_empty(out: &mut Out<'_>, ctx: &ObjectCtx<'_>) {
        if !ctx.inline_open {
            out.push_indent(ctx.depth);
        }
        out.push_str("{}");
    }
}

pub(super) fn render_array(ctx: &ArrayCtx, out: &mut Out<'_>) {
    render_array_with::<Json>(ctx, out)
}

pub(super) fn render_object(ctx: &ObjectCtx<'_>, out: &mut Out<'_>) {
    render_object_with::<Json>(ctx, out)
}
