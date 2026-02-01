use super::core::{
    Style, push_array_items_with, push_object_items, wrap_block,
};
use super::{ArrayCtx, ObjectCtx};
use crate::serialization::output::Out;

struct Pseudo;

impl Style for Pseudo {
    fn array_push_omitted(out: &mut Out<'_>, ctx: &ArrayCtx<'_>) {
        if ctx.omitted > 0 {
            out.push_indent(ctx.depth + 1);
            out.push_omission();
            if ctx.children_len > 0 && ctx.omitted_at_start {
                out.push_char(',');
            }
            out.push_newline();
        }
    }
    fn array_push_internal_gap(
        out: &mut Out<'_>,
        ctx: &ArrayCtx<'_>,
        _gap: usize,
    ) {
        out.push_indent(ctx.depth + 1);
        out.push_omission();
        out.push_newline();
    }

    fn object_push_omitted(out: &mut Out<'_>, ctx: &ObjectCtx<'_>) {
        if ctx.omitted > 0 {
            out.push_indent(ctx.depth + 1);
            out.push_omission();
            out.push_newline();
        }
    }
}

fn render_array_empty(ctx: &ArrayCtx<'_>, out: &mut Out<'_>) {
    if !ctx.inline_open {
        out.push_indent(ctx.depth);
    }
    out.push_char('[');
    if ctx.omitted > 0 {
        out.push_str(" ");
        out.push_omission();
        out.push_str(" ");
    }
    out.push_char(']');
}

fn render_array_nonempty(ctx: &ArrayCtx<'_>, out: &mut Out<'_>) {
    wrap_block(out, ctx.depth, ctx.inline_open, '[', ']', |o| {
        if ctx.omitted_at_start {
            <Pseudo as Style>::array_push_omitted(o, ctx);
        }
        push_array_items_with::<Pseudo>(o, ctx);
        if !ctx.omitted_at_start {
            <Pseudo as Style>::array_push_omitted(o, ctx);
        }
    });
}

// JSONL rendering is intentionally duplicated across js/pseudo templates for simplicity.
// See also: push_jsonl_gap and render_jsonl_root in js.rs.
fn push_jsonl_gap(
    out: &mut Out<'_>,
    ctx: &ArrayCtx<'_>,
    prev_index: Option<usize>,
    orig_index: usize,
) {
    if let Some(prev) = prev_index {
        if orig_index > prev.saturating_add(1) {
            out.push_indent(ctx.depth);
            out.push_omission();
            out.push_newline();
        }
    } else if ctx.omitted_at_start && ctx.omitted > 0 {
        out.push_indent(ctx.depth);
        out.push_omission();
        out.push_newline();
    }
}

fn render_jsonl_root(ctx: &ArrayCtx<'_>, out: &mut Out<'_>) {
    let max_line = ctx.children.iter().map(|(idx, _)| *idx).max().unwrap_or(0);
    let width = format!("{max_line}").len();
    let mut prev_index: Option<usize> = None;
    for (i, (orig_index, (_kind, item))) in ctx.children.iter().enumerate() {
        push_jsonl_gap(out, ctx, prev_index, *orig_index);
        out.push_str(&format!("{orig_index:>width$}: "));
        out.push_str(item);
        if i + 1 < ctx.children_len {
            out.push_newline();
        }
        prev_index = Some(*orig_index);
    }
    if !ctx.omitted_at_start && ctx.omitted > 0 {
        out.push_newline();
        out.push_indent(ctx.depth);
        out.push_omission();
    }
}

pub(super) fn render_array(ctx: &ArrayCtx<'_>, out: &mut Out<'_>) {
    if ctx.is_jsonl_root && !out.is_compact_mode() {
        if ctx.children_len == 0 && ctx.omitted > 0 {
            out.push_omission();
        } else if ctx.children_len > 0 {
            render_jsonl_root(ctx, out);
        }
        return;
    }
    if ctx.children_len == 0 {
        render_array_empty(ctx, out);
    } else {
        render_array_nonempty(ctx, out);
    }
}

pub(super) fn render_object(ctx: &ObjectCtx<'_>, out: &mut Out<'_>) {
    if ctx.children_len == 0 {
        if !ctx.inline_open {
            out.push_indent(ctx.depth);
        }
        out.push_char('{');
        if ctx.omitted > 0 {
            out.push_str(ctx.space);
            out.push_omission();
            out.push_str(ctx.space);
        }
        out.push_char('}');
        return;
    }
    wrap_block(out, ctx.depth, ctx.inline_open, '{', '}', |o| {
        push_object_items(o, ctx);
        <Pseudo as Style>::object_push_omitted(o, ctx);
    });
}
