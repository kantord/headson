use super::super::indent;
use super::{ArrayCtx, ObjectCtx};

fn array_empty(open_indent: &str) -> String {
    format!("{open_indent}[]")
}

fn object_empty(open_indent: &str) -> String {
    // Print an empty object: "{}"
    format!("{open_indent}{{}}")
}

pub fn render_array(ctx: &ArrayCtx) -> String {
    let base = indent(ctx.depth, &ctx.indent_unit);
    let open_indent = if ctx.inline_open { "" } else { &base };
    if ctx.children_len == 0 {
        return array_empty(open_indent);
    }
    let mut out = String::new();
    out.push_str(open_indent);
    out.push('[');
    out.push_str(&ctx.newline);
    for (i, (_, item)) in ctx.children.iter().enumerate() {
        out.push_str(item);
        if i + 1 < ctx.children_len {
            out.push(',');
        }
        out.push_str(&ctx.newline);
    }
    out.push_str(&base);
    out.push(']');
    out
}

pub fn render_object(ctx: &ObjectCtx) -> String {
    let base = indent(ctx.depth, &ctx.indent_unit);
    let open_indent = if ctx.inline_open { "" } else { &base };
    if ctx.children_len == 0 {
        return object_empty(open_indent);
    }
    let mut out = String::new();
    out.push_str(open_indent);
    out.push('{');
    out.push_str(&ctx.newline);
    for (i, (_, (k, v))) in ctx.children.iter().enumerate() {
        out.push_str(&indent(ctx.depth + 1, &ctx.indent_unit));
        out.push_str(k);
        out.push(':');
        out.push_str(&ctx.space);
        out.push_str(v);
        if i + 1 < ctx.children_len {
            out.push(',');
        }
        out.push_str(&ctx.newline);
    }
    out.push_str(&base);
    out.push('}');
    out
}
