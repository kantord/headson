use super::core::{
    Style, push_array_items_with, push_object_items, wrap_block,
};
use super::{ArrayCtx, ObjectCtx};
use crate::serialization::output::Out;

struct Js;

impl Style for Js {
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

fn render_array_empty(ctx: &ArrayCtx, out: &mut Out<'_>) {
    if !ctx.inline_open {
        out.push_indent(ctx.depth);
    }
    out.push_char('[');
    if ctx.omitted > 0 {
        out.push_str(" ");
        out.push_comment(format!("/* {} more items */", ctx.omitted));
        out.push_str(" ");
    }
    out.push_char(']');
}

fn render_array_nonempty(ctx: &ArrayCtx, out: &mut Out<'_>) {
    wrap_block(out, ctx.depth, ctx.inline_open, '[', ']', |o| {
        if ctx.omitted_at_start {
            <Js as Style>::array_push_omitted(o, ctx);
        }
        push_array_items_with::<Js>(o, ctx);
        if !ctx.omitted_at_start {
            <Js as Style>::array_push_omitted(o, ctx);
        }
    });
}

pub(super) fn render_array(ctx: &ArrayCtx, out: &mut Out<'_>) {
    if ctx.children_len == 0 {
        render_array_empty(ctx, out);
    } else {
        render_array_nonempty(ctx, out);
    }
}

fn render_object_empty(ctx: &ObjectCtx<'_>, out: &mut Out<'_>) {
    if !ctx.inline_open {
        out.push_indent(ctx.depth);
    }
    out.push_char('{');
    if ctx.omitted > 0 {
        out.push_str(ctx.space);
        let label = if ctx.fileset_root {
            "files"
        } else {
            "properties"
        };
        out.push_comment(format!("/* {} more {label} */", ctx.omitted));
        out.push_str(ctx.space);
    }
    out.push_char('}');
}

fn render_object_nonempty(ctx: &ObjectCtx<'_>, out: &mut Out<'_>) {
    wrap_block(out, ctx.depth, ctx.inline_open, '{', '}', |o| {
        push_object_items(o, ctx);
        <Js as Style>::object_push_omitted(o, ctx);
    });
}

pub(super) fn render_object(ctx: &ObjectCtx<'_>, out: &mut Out<'_>) {
    if ctx.children_len == 0 {
        render_object_empty(ctx, out);
    } else {
        render_object_nonempty(ctx, out);
    }
}
