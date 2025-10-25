use super::super::indent;
use super::core::{Style, render_array_with, render_object_with};
use super::{ArrayCtx, ObjectCtx};

struct Pseudo;

impl Style for Pseudo {
    fn array_empty(open_indent: &str, ctx: &ArrayCtx<'_>) -> String {
        if ctx.omitted > 0 {
            return format!("{open_indent}[ … ]");
        }
        format!("{open_indent}[]")
    }

    fn array_push_omitted(out: &mut String, ctx: &ArrayCtx<'_>) {
        if ctx.omitted > 0 {
            out.push_str(&indent(ctx.depth + 1, ctx.indent_unit));
            out.push('…');
            if ctx.children_len > 0 && ctx.omitted_at_start {
                out.push(',');
            }
            out.push_str(ctx.newline);
        }
    }

    fn object_empty(open_indent: &str, ctx: &ObjectCtx<'_>) -> String {
        if ctx.omitted > 0 {
            return format!(
                "{open_indent}{{{space}…{space}}}",
                space = ctx.space
            );
        }
        format!("{open_indent}{{}}")
    }

    fn object_push_omitted(out: &mut String, ctx: &ObjectCtx<'_>) {
        if ctx.omitted > 0 {
            out.push_str(&indent(ctx.depth + 1, ctx.indent_unit));
            out.push('…');
            out.push_str(ctx.newline);
        }
    }
}

pub(super) fn render_array(ctx: &ArrayCtx<'_>) -> String {
    render_array_with::<Pseudo>(ctx)
}

pub(super) fn render_object(ctx: &ObjectCtx<'_>) -> String {
    render_object_with::<Pseudo>(ctx)
}
