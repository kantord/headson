use super::super::indent;
use super::core::{Style, render_array_with, render_object_with};
use super::{ArrayCtx, ObjectCtx};

struct Js;

impl Style for Js {
    fn array_empty(open_indent: &str, ctx: &ArrayCtx<'_>) -> String {
        if ctx.omitted > 0 {
            let body = format!("/* {} more items */", ctx.omitted);
            let body = if ctx.color_enabled {
                format!("\u{001b}[90m{body}\u{001b}[0m")
            } else {
                body
            };
            return format!("{open_indent}[ {body} ]");
        }
        let body = if ctx.color_enabled {
            "\u{001b}[90m/* empty */\u{001b}[0m"
        } else {
            "/* empty */"
        };
        format!("{open_indent}[ {body} ]")
    }

    fn array_push_omitted(out: &mut String, ctx: &ArrayCtx<'_>) {
        if ctx.omitted > 0 {
            out.push_str(&indent(ctx.depth + 1, ctx.indent_unit));
            if ctx.color_enabled {
                out.push_str("\u{001b}[90m");
            }
            out.push_str("/* ");
            out.push_str(&ctx.omitted.to_string());
            out.push_str(" more items */");
            if ctx.color_enabled {
                out.push_str("\u{001b}[0m");
            }
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
        out.push_str(&indent(ctx.depth + 1, ctx.indent_unit));
        if ctx.color_enabled {
            out.push_str("\u{001b}[90m");
        }
        out.push_str("/* ");
        out.push_str(&gap.to_string());
        out.push_str(" more items */");
        if ctx.color_enabled {
            out.push_str("\u{001b}[0m");
        }
        out.push_str(ctx.newline);
    }

    fn object_empty(open_indent: &str, ctx: &ObjectCtx<'_>) -> String {
        if ctx.omitted > 0 {
            let label = if ctx.fileset_root {
                "files"
            } else {
                "properties"
            };
            let body = format!("/* {} more {label} */", ctx.omitted);
            let body = if ctx.color_enabled {
                format!("\u{001b}[90m{body}\u{001b}[0m")
            } else {
                body
            };
            return format!(
                "{open_indent}{{{space}{body}{space}}}",
                space = ctx.space
            );
        }
        let body = if ctx.color_enabled {
            "\u{001b}[90m/* empty */\u{001b}[0m"
        } else {
            "/* empty */"
        };
        format!("{open_indent}{{{space}{body}{space}}}", space = ctx.space)
    }

    fn object_push_omitted(out: &mut String, ctx: &ObjectCtx<'_>) {
        if ctx.omitted > 0 {
            out.push_str(&indent(ctx.depth + 1, ctx.indent_unit));
            let label = if ctx.fileset_root {
                "files"
            } else {
                "properties"
            };
            if ctx.color_enabled {
                out.push_str("\u{001b}[90m");
            }
            out.push_str(&format!("/* {} more {label} */", ctx.omitted));
            if ctx.color_enabled {
                out.push_str("\u{001b}[0m");
            }
            out.push_str(ctx.newline);
        }
    }
}

pub(super) fn render_array(ctx: &ArrayCtx<'_>) -> String {
    render_array_with::<Js>(ctx)
}

pub(super) fn render_object(ctx: &ObjectCtx<'_>) -> String {
    render_object_with::<Js>(ctx)
}
