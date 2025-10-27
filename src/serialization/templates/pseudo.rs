use super::core::{Style, render_array_with, render_object_with};
use super::{ArrayCtx, ObjectCtx};
use crate::serialization::output::Out;

struct Pseudo;

impl Style for Pseudo {
    fn array_empty(open_indent: &str, ctx: &ArrayCtx<'_>) -> String {
        let mut buf = String::new();
        let mut out = Out::from_array_ctx(&mut buf, ctx);
        out.push_str(open_indent);
        out.push_char('[');
        if ctx.omitted > 0 {
            out.push_str(" ");
            out.push_omission();
            out.push_str(" ");
        }
        out.push_char(']');
        buf
    }

    fn array_push_omitted(out: &mut String, ctx: &ArrayCtx<'_>) {
        if ctx.omitted > 0 {
            let mut ow = Out::from_array_ctx(out, ctx);
            ow.push_indent(ctx.depth + 1);
            ow.push_omission();
            if ctx.children_len > 0 && ctx.omitted_at_start {
                ow.push_char(',');
            }
            ow.push_newline();
        }
    }
    fn array_push_internal_gap(
        out: &mut String,
        ctx: &ArrayCtx<'_>,
        _gap: usize,
    ) {
        let mut ow = Out::from_array_ctx(out, ctx);
        ow.push_indent(ctx.depth + 1);
        ow.push_omission();
        ow.push_newline();
    }

    fn object_empty(open_indent: &str, ctx: &ObjectCtx<'_>) -> String {
        let mut buf = String::new();
        let mut out = Out::from_object_ctx(&mut buf, ctx);
        out.push_str(open_indent);
        out.push_char('{');
        if ctx.omitted > 0 {
            out.push_str(ctx.space);
            out.push_omission();
            out.push_str(ctx.space);
        }
        out.push_char('}');
        buf
    }

    fn object_push_omitted(out: &mut String, ctx: &ObjectCtx<'_>) {
        if ctx.omitted > 0 {
            let mut ow = Out::from_object_ctx(out, ctx);
            ow.push_indent(ctx.depth + 1);
            ow.push_omission();
            ow.push_newline();
        }
    }
}

pub(super) fn render_array(ctx: &ArrayCtx<'_>) -> String {
    render_array_with::<Pseudo>(ctx)
}

pub(super) fn render_object(ctx: &ObjectCtx<'_>) -> String {
    render_object_with::<Pseudo>(ctx)
}
