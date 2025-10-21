use super::super::indent;
use super::{ArrayCtx, ObjectCtx};

pub trait Style {
    fn array_empty(open_indent: &str, ctx: &ArrayCtx) -> String;
    fn array_push_omitted(_out: &mut String, _ctx: &ArrayCtx) {}

    fn object_empty(open_indent: &str, ctx: &ObjectCtx) -> String;
    fn object_push_omitted(_out: &mut String, _ctx: &ObjectCtx) {}
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

pub fn render_array_with<S: Style>(ctx: &ArrayCtx) -> String {
    let base = indent(ctx.depth, &ctx.indent_unit);
    let open_indent = if ctx.inline_open { "" } else { &base };
    if ctx.children_len == 0 {
        return S::array_empty(open_indent, ctx);
    }
    let mut out = String::new();
    out.push_str(open_indent);
    out.push('[');
    out.push_str(&ctx.newline);
    push_array_items(&mut out, ctx);
    S::array_push_omitted(&mut out, ctx);
    out.push_str(&base);
    out.push(']');
    out
}

pub fn render_object_with<S: Style>(ctx: &ObjectCtx) -> String {
    let base = indent(ctx.depth, &ctx.indent_unit);
    let open_indent = if ctx.inline_open { "" } else { &base };
    if ctx.children_len == 0 {
        return S::object_empty(open_indent, ctx);
    }
    let mut out = String::new();
    out.push_str(open_indent);
    out.push('{');
    out.push_str(&ctx.newline);
    push_object_items(&mut out, ctx);
    S::object_push_omitted(&mut out, ctx);
    out.push_str(&base);
    out.push('}');
    out
}
