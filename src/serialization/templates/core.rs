use super::{ArrayCtx, ObjectCtx};
use crate::serialization::output::Out;

// Shared rendering core for all templates.
// - Style controls only empty/omitted decorations.
// - Indentation and newlines come from ctx (depth, indent_unit, newline).
// - When ctx.inline_open is true, no leading indent is emitted before the opener.
pub trait Style {
    fn array_empty(out: &mut Out<'_>, ctx: &ArrayCtx);
    fn array_push_omitted(_out: &mut Out<'_>, _ctx: &ArrayCtx) {}
    fn array_push_internal_gap(
        _out: &mut Out<'_>,
        _ctx: &ArrayCtx,
        _gap: usize,
    ) {
    }

    fn object_empty(out: &mut Out<'_>, ctx: &ObjectCtx<'_>);
    fn object_push_omitted(_out: &mut Out<'_>, _ctx: &ObjectCtx<'_>) {}
}

fn push_array_items_with<S: Style>(out: &mut Out<'_>, ctx: &ArrayCtx) {
    let mut prev_index: Option<usize> = None;
    for (i, (orig_index, item)) in ctx.children.iter().enumerate() {
        if let Some(prev) = prev_index {
            if *orig_index > prev.saturating_add(1) {
                S::array_push_internal_gap(out, ctx, *orig_index - prev - 1);
            }
        }
        out.push_str(item);
        if i + 1 < ctx.children_len {
            out.push_char(',');
        }
        out.push_newline();
        prev_index = Some(*orig_index);
    }
}

#[inline]
fn as_bool(v: &str) -> Option<bool> {
    if v == "true" {
        Some(true)
    } else if v == "false" {
        Some(false)
    } else {
        None
    }
}

#[inline]
fn is_number_text(v: &str) -> bool {
    matches!(v.as_bytes().first().copied(), Some(b'-' | b'0'..=b'9'))
}

#[inline]
fn push_value_token(out: &mut Out<'_>, v: &str) {
    if v.starts_with('"') {
        out.push_string_literal(v);
        return;
    }
    if let Some(b) = as_bool(v) {
        out.push_bool(b);
        return;
    }
    if v == "null" {
        out.push_null();
        return;
    }
    if is_number_text(v) {
        out.push_number_literal(v);
        return;
    }
    out.push_str(v);
}

fn push_object_items(out: &mut Out<'_>, ctx: &ObjectCtx<'_>) {
    for (i, (_, (k, v))) in ctx.children.iter().enumerate() {
        out.push_indent(ctx.depth + 1);
        out.push_key(k);
        out.push_char(':');
        out.push_str(ctx.space);
        push_value_token(out, v);
        if i + 1 < ctx.children_len {
            out.push_char(',');
        }
        out.push_newline();
    }
}

// Render an array using the shared control flow and style-specific decorations.
pub fn render_array_with<S: Style>(ctx: &ArrayCtx, out: &mut Out<'_>) {
    if ctx.children_len == 0 {
        S::array_empty(out, ctx);
        return;
    }
    if !ctx.inline_open {
        out.push_indent(ctx.depth);
    }
    out.push_char('[');
    out.push_newline();
    if ctx.omitted_at_start {
        S::array_push_omitted(out, ctx);
    }
    push_array_items_with::<S>(out, ctx);
    if !ctx.omitted_at_start {
        S::array_push_omitted(out, ctx);
    }
    out.push_indent(ctx.depth);
    out.push_char(']');
}

// Render an object using the shared control flow and style-specific decorations.
pub fn render_object_with<S: Style>(ctx: &ObjectCtx<'_>, out: &mut Out<'_>) {
    if ctx.children_len == 0 {
        S::object_empty(out, ctx);
        return;
    }
    if !ctx.inline_open {
        out.push_indent(ctx.depth);
    }
    out.push_char('{');
    out.push_newline();
    push_object_items(out, ctx);
    S::object_push_omitted(out, ctx);
    out.push_indent(ctx.depth);
    out.push_char('}');
}
