use super::{ArrayCtx, ObjectCtx};

pub fn render_array(ctx: &ArrayCtx) -> String {
    if ctx.children_len == 0 {
        if ctx.omitted > 0 {
            return format!(
                "{}[ /* {} more items */ ]",
                ctx.indent0, ctx.omitted
            );
        } else {
            return format!("{}[ /* empty */ ]", ctx.indent0);
        }
    }
    let mut out = String::new();
    out.push_str(&ctx.indent0);
    out.push('[');
    out.push('\n');
    for (i, (_, item)) in ctx.children.iter().enumerate() {
        out.push_str(item);
        if i + 1 < ctx.children_len {
            out.push(',');
        }
        out.push('\n');
    }
    if ctx.omitted > 0 {
        out.push_str(&ctx.indent1);
        out.push_str(&format!("/* {} more items */\n", ctx.omitted));
    }
    out.push_str(&ctx.indent0);
    out.push(']');
    out
}

pub fn render_object(ctx: &ObjectCtx) -> String {
    if ctx.children_len == 0 {
        if ctx.omitted > 0 {
            return format!(
                "{}{{{}/* {} more properties */{}}}",
                ctx.indent0, ctx.sp, ctx.omitted, ctx.sp
            );
        } else {
            return format!(
                "{}{{{}/* empty */{}}}",
                ctx.indent0, ctx.sp, ctx.sp
            );
        }
    }
    let mut out = String::new();
    out.push_str(&ctx.indent0);
    out.push('{');
    out.push('\n');
    for (i, (_, (k, v))) in ctx.children.iter().enumerate() {
        out.push_str(&ctx.indent1);
        out.push_str(k);
        out.push(':');
        out.push_str(&ctx.sp);
        out.push_str(v);
        if i + 1 < ctx.children_len {
            out.push(',');
        }
        out.push('\n');
    }
    if ctx.omitted > 0 {
        out.push_str(&ctx.indent1);
        out.push_str(&format!("/* {} more properties */\n", ctx.omitted));
    }
    out.push_str(&ctx.indent0);
    out.push('}');
    out
}
