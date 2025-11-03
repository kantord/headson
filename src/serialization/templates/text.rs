use super::{ArrayCtx, ObjectCtx};
use crate::serialization::output::Out;

fn push_text_omission_line(out: &mut Out<'_>, omitted: usize) {
    match out.style() {
        crate::serialization::types::Style::Strict => {}
        crate::serialization::types::Style::Default => {
            out.push_omission();
            out.push_newline();
        }
        crate::serialization::types::Style::Detailed => {
            out.push_omission();
            out.push_str(" ");
            out.push_str(&format!("{omitted} more lines "));
            out.push_omission();
            out.push_newline();
        }
    }
}

pub(super) fn render_array(ctx: &ArrayCtx, out: &mut Out<'_>) {
    // For text, arrays are treated as raw lines of text. We do not emit
    // brackets or indentation; we only write lines and optional omission markers.
    if ctx.omitted_at_start && ctx.omitted > 0 {
        push_text_omission_line(out, ctx.omitted);
    }
    for (_, (_, item)) in ctx.children.iter() {
        out.push_str(item);
        out.push_newline();
    }
    if !ctx.omitted_at_start && ctx.omitted > 0 {
        push_text_omission_line(out, ctx.omitted);
    }
}

pub(super) fn render_object(ctx: &ObjectCtx<'_>, out: &mut Out<'_>) {
    // Special-case indent-ingest nodes: objects of shape { line: <str>, children: <array> }.
    // Render as raw text with indentation derived from depth.
    let mut line_val: Option<&str> = None;
    let mut children_block: Option<&str> = None;
    for (_, (k, v)) in ctx.children.iter() {
        if k == "line" {
            line_val = Some(v.as_str());
        } else if k == "children" {
            children_block = Some(v.as_str());
        }
    }
    if let Some(line) = line_val {
        // Render current line
        out.push_indent(ctx.depth);
        out.push_str(line);
        out.push_newline();
        // Render children lines, each prefixed with one extra indent level
        if let Some(block) = children_block {
            if !block.is_empty() {
                let mut start = 0usize;
                let bytes = block.as_bytes();
                for (i, &b) in bytes.iter().enumerate() {
                    if b == b'\n' {
                        let slice = &block[start..i];
                        if !slice.is_empty() {
                            out.push_indent(ctx.depth + 1);
                            out.push_str(slice);
                        }
                        out.push_newline();
                        start = i + 1;
                    }
                }
                // Handle trailing fragment without newline (unlikely, but safe)
                if start < bytes.len() {
                    let slice = &block[start..];
                    if !slice.is_empty() {
                        out.push_indent(ctx.depth + 1);
                        out.push_str(slice);
                        out.push_newline();
                    }
                }
            }
        }
        return;
    }
    // Fallback: delegate to pseudo renderer if not an indent node.
    super::pseudo::render_object(ctx, out);
}
