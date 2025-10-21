use super::{ArrayCtx, ObjectCtx};

pub fn render_array(ctx: &ArrayCtx) -> String {
    if ctx.children_len == 0 {
        return format!("{}[]", ctx.indent0);
    }
    let mut out = String::new();
    out.push_str(&ctx.indent0);
    out.push('[');
    out.push_str(&ctx.nl);
    for (i, (_, item)) in ctx.children.iter().enumerate() {
        out.push_str(item);
        if i + 1 < ctx.children_len {
            out.push(',');
        }
        out.push_str(&ctx.nl);
    }
    out.push_str(&ctx.indent0);
    out.push(']');
    out
}

pub fn render_object(ctx: &ObjectCtx) -> String {
    if ctx.children_len == 0 {
        return format!("{}{}", ctx.indent0, "{}");
    }
    let mut out = String::new();
    out.push_str(&ctx.indent0);
    out.push('{');
    out.push_str(&ctx.nl);
    for (i, (_, (k, v))) in ctx.children.iter().enumerate() {
        out.push_str(&ctx.indent1);
        out.push_str(k);
        out.push(':');
        out.push_str(&ctx.sp);
        out.push_str(v);
        if i + 1 < ctx.children_len {
            out.push(',');
        }
        out.push_str(&ctx.nl);
    }
    out.push_str(&ctx.indent0);
    out.push('}');
    out
}
