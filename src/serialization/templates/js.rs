use super::core::{Style, render_array_with, render_object_with};
use super::{ArrayCtx, ObjectCtx};
use crate::serialization::output::Out;

struct Js;

impl Style for Js {
    fn array_empty(out: &mut Out<'_>, ctx: &ArrayCtx) {
        if !ctx.inline_open {
            out.push_indent(ctx.depth);
        }
        out.push_char('[');
        out.push_str(" ");
        if ctx.omitted > 0 {
            out.push_comment(format!("/* {} more items */", ctx.omitted));
        } else {
            out.push_comment("/* empty */");
        }
        out.push_str(" ");
        out.push_char(']');
    }

    fn array_push_omitted(out: &mut Out<'_>, ctx: &ArrayCtx) {
        if ctx.omitted > 0 {
            out.push_indent(ctx.depth + 1);
            out.push_comment(format!("/* {} more items */", ctx.omitted));
            if ctx.children_len > 0 && ctx.omitted_at_start {
                out.push_char(',');
            }
            out.push_newline();
        }
    }
    fn array_push_internal_gap(out: &mut Out<'_>, ctx: &ArrayCtx, gap: usize) {
        out.push_indent(ctx.depth + 1);
        out.push_comment(format!("/* {gap} more items */"));
        out.push_newline();
    }

    fn object_empty(out: &mut Out<'_>, ctx: &ObjectCtx<'_>) {
        if !ctx.inline_open {
            out.push_indent(ctx.depth);
        }
        out.push_char('{');
        out.push_str(ctx.space);
        if ctx.omitted > 0 {
            let label = if ctx.fileset_root {
                "files"
            } else {
                "properties"
            };
            out.push_comment(format!("/* {} more {label} */", ctx.omitted));
        } else {
            out.push_comment("/* empty */");
        }
        out.push_str(ctx.space);
        out.push_char('}');
    }

    fn object_push_omitted(out: &mut Out<'_>, ctx: &ObjectCtx<'_>) {
        if ctx.omitted > 0 {
            out.push_indent(ctx.depth + 1);
            let label = if ctx.fileset_root {
                "files"
            } else {
                "properties"
            };
            out.push_comment(format!("/* {} more {label} */", ctx.omitted));
            out.push_newline();
        }
    }
}

pub(super) fn render_array(ctx: &ArrayCtx, out: &mut Out<'_>) {
    render_array_with::<Js>(ctx, out)
}

pub(super) fn render_object(ctx: &ObjectCtx<'_>, out: &mut Out<'_>) {
    render_object_with::<Js>(ctx, out)
}
