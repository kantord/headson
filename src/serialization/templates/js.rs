use super::super::indent;
use super::core::{Style, render_array_with, render_object_with};
use super::{ArrayCtx, ObjectCtx};

struct Js;

impl Style for Js {
    fn array_empty(open_indent: &str, ctx: &ArrayCtx<'_>) -> String {
        if ctx.omitted > 0 {
            return format!(
                "{open_indent}[ /* {} more items */ ]",
                ctx.omitted
            );
        }
        format!("{open_indent}[ /* empty */ ]")
    }

    fn array_push_omitted(out: &mut String, ctx: &ArrayCtx<'_>) {
        if ctx.omitted > 0 {
            out.push_str(&indent(ctx.depth + 1, ctx.indent_unit));
            out.push_str("/* ");
            out.push_str(&ctx.omitted.to_string());
            out.push_str(" more items */");
            if ctx.children_len > 0 && ctx.omitted_at_start {
                out.push(',');
            }
            out.push_str(ctx.newline);
        }
    }
    fn array_push_internal_gap(
        out: &mut String,
        ctx: &ArrayCtx<'_>,
        gap: usize,
    ) {
        if ctx.newline.is_empty() {
            // In compact mode, suppress internal gap comments to keep
            // a single omitted summary for easier machine parsing.
            return;
        }
        out.push_str(&indent(ctx.depth + 1, ctx.indent_unit));
        out.push_str("/* ");
        out.push_str(&gap.to_string());
        out.push_str(" more items */");
        out.push_str(ctx.newline);
    }

    fn object_empty(open_indent: &str, ctx: &ObjectCtx<'_>) -> String {
        if ctx.omitted > 0 {
            let label = if ctx.fileset_root {
                "files"
            } else {
                "properties"
            };
            return format!(
                "{open_indent}{{{space}/* {n} more {label} */{space}}}",
                n = ctx.omitted,
                space = ctx.space
            );
        }
        format!(
            "{open_indent}{{{space}/* empty */{space}}}",
            space = ctx.space
        )
    }

    fn object_push_omitted(out: &mut String, ctx: &ObjectCtx<'_>) {
        if ctx.omitted > 0 {
            out.push_str(&indent(ctx.depth + 1, ctx.indent_unit));
            let label = if ctx.fileset_root {
                "files"
            } else {
                "properties"
            };
            out.push_str(&format!(
                "/* {} more {label} */{}",
                ctx.omitted, ctx.newline
            ));
        }
    }
}

pub(super) fn render_array(ctx: &ArrayCtx<'_>) -> String {
    render_array_with::<Js>(ctx)
}

pub(super) fn render_object(ctx: &ObjectCtx<'_>) -> String {
    render_object_with::<Js>(ctx)
}
