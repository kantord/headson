use super::super::indent;
use super::{ArrayCtx, ObjectCtx};

fn array_empty(open_indent: &str, omitted: usize) -> String {
    if omitted > 0 {
        return format!("{open_indent}[ … ]");
    }
    format!("{open_indent}[]")
}

fn object_empty(open_indent: &str, space: &str, omitted: usize) -> String {
    if omitted > 0 {
        return format!("{open_indent}{{{space}…{space}}}");
    }
    format!("{open_indent}{{}}")
}

fn push_array_items(out: &mut String, ctx: &ArrayCtx) {
    for (i, (_, item)) in ctx.children.iter().enumerate() {
        out.push_str(item);
        if i + 1 < ctx.children_len {
            out.push(',');
        }
        out.push_str(&ctx.newline);
    }
}

fn push_array_omitted(out: &mut String, ctx: &ArrayCtx) {
    if ctx.omitted > 0 {
        out.push_str(&indent(ctx.depth + 1, &ctx.indent_unit));
        out.push('…');
        out.push_str(&ctx.newline);
    }
}

pub fn render_array(ctx: &ArrayCtx) -> String {
    let base = indent(ctx.depth, &ctx.indent_unit);
    let open_indent = if ctx.inline_open { "" } else { &base };
    if ctx.children_len == 0 {
        return array_empty(open_indent, ctx.omitted);
    }
    let mut out = String::new();
    out.push_str(open_indent);
    out.push('[');
    out.push_str(&ctx.newline);
    push_array_items(&mut out, ctx);
    push_array_omitted(&mut out, ctx);
    out.push_str(&base);
    out.push(']');
    out
}

fn push_object_items(out: &mut String, ctx: &ObjectCtx) {
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
}

fn push_object_omitted(out: &mut String, ctx: &ObjectCtx) {
    if ctx.omitted > 0 {
        out.push_str(&indent(ctx.depth + 1, &ctx.indent_unit));
        out.push('…');
        out.push_str(&ctx.newline);
    }
}

pub fn render_object(ctx: &ObjectCtx) -> String {
    let base = indent(ctx.depth, &ctx.indent_unit);
    let open_indent = if ctx.inline_open { "" } else { &base };
    if ctx.children_len == 0 {
        return object_empty(open_indent, &ctx.space, ctx.omitted);
    }
    let mut out = String::new();
    out.push_str(open_indent);
    out.push('{');
    out.push_str(&ctx.newline);
    push_object_items(&mut out, ctx);
    push_object_omitted(&mut out, ctx);
    out.push_str(&base);
    out.push('}');
    out
}
