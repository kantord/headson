use super::{ArrayCtx, ObjectCtx};
use crate::order::NodeKind;
use crate::serialization::output::Out;

// Shared rendering core for all templates.
// - Style controls only empty/omitted decorations.
// - Indentation and newlines come from ctx (depth, indent_unit, newline).
// - When ctx.inline_open is true, no leading indent is emitted before the opener.
pub trait Style {
    fn array_push_omitted(_out: &mut Out<'_>, _ctx: &ArrayCtx) {}
    fn array_push_internal_gap(
        _out: &mut Out<'_>,
        _ctx: &ArrayCtx,
        _gap: usize,
    ) {
    }
    fn object_push_omitted(_out: &mut Out<'_>, _ctx: &ObjectCtx<'_>) {}
}

fn has_any_newline(s: &str) -> bool {
    s.as_bytes().contains(&b'\n') || s.as_bytes().contains(&b'\r')
}

fn maybe_push_internal_gap<S: Style>(
    out: &mut Out<'_>,
    ctx: &ArrayCtx,
    prev_index: Option<usize>,
    orig_index: usize,
) {
    if let Some(prev) = prev_index {
        if orig_index > prev.saturating_add(1) {
            S::array_push_internal_gap(out, ctx, orig_index - prev - 1);
        }
    }
}

fn push_single_array_item(
    out: &mut Out<'_>,
    ctx: &ArrayCtx,
    kind: NodeKind,
    item: &str,
) {
    if has_any_newline(item) {
        out.push_str(item);
        return;
    }
    match kind {
        NodeKind::Array | NodeKind::Object => out.push_str(item),
        _ => {
            out.push_indent(ctx.depth + 1);
            out.push_str(item);
        }
    }
}

pub(crate) fn push_array_items_with<S: Style>(
    out: &mut Out<'_>,
    ctx: &ArrayCtx,
) {
    let mut prev_index: Option<usize> = None;
    for (i, (orig_index, (kind, item))) in ctx.children.iter().enumerate() {
        maybe_push_internal_gap::<S>(out, ctx, prev_index, *orig_index);
        push_single_array_item(out, ctx, *kind, item);
        if i + 1 < ctx.children_len {
            out.push_char(',');
        }
        out.push_newline();
        prev_index = Some(*orig_index);
    }
}

fn push_value_token(out: &mut Out<'_>, v: &str) {
    // Preserve exact token text; only color string literals.
    if v.starts_with('"') {
        out.push_string_literal(v);
    } else {
        out.push_str(v);
    }
}

pub(crate) fn push_object_items(out: &mut Out<'_>, ctx: &ObjectCtx<'_>) {
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

// A no-op style for cases where only the array item printing is desired without gap markers.
pub struct StyleNoop;
impl Style for StyleNoop {}

// Combinators and tiny building blocks

pub(crate) fn open_block(
    out: &mut Out<'_>,
    depth: usize,
    inline: bool,
    ch: char,
) {
    if !inline {
        out.push_indent(depth);
    }
    out.push_char(ch);
}

pub(crate) fn close_block(out: &mut Out<'_>, depth: usize, ch: char) {
    out.push_indent(depth);
    out.push_char(ch);
}

pub(crate) fn wrap_block(
    out: &mut Out<'_>,
    depth: usize,
    inline: bool,
    open_ch: char,
    close_ch: char,
    body: impl FnOnce(&mut Out<'_>),
) {
    open_block(out, depth, inline, open_ch);
    out.push_newline();
    body(out);
    close_block(out, depth, close_ch);
}
