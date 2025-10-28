use super::ArrayCtx;
use super::ObjectCtx;
use crate::serialization::output::Out;
use serde_json;

fn has_newline(s: &str) -> bool {
    s.as_bytes().contains(&b'\n') || s.contains('\r')
}

fn push_yaml_array_item(out: &mut Out<'_>, depth: usize, item: &str) {
    if !has_newline(item) {
        out.push_indent(depth);
        out.push_str("- ");
        out.push_str(item.trim());
        out.push_newline();
        return;
    }
    let mut lines = item.split_inclusive(&['\n', '\r'][..]);
    if let Some(first) = lines.next() {
        let first_trimmed = first.trim_start_matches(['\n', '\r']);
        out.push_indent(depth);
        out.push_str("- ");
        out.push_str(first_trimmed);
    }
    for rest in lines {
        out.push_str(rest);
    }
    if !item.ends_with('\n') && !item.ends_with('\r') {
        out.push_newline();
    }
}

fn push_array_omitted_start(ctx: &ArrayCtx, out: &mut Out<'_>) {
    if ctx.omitted_at_start && ctx.omitted > 0 {
        out.push_indent(ctx.depth);
        out.push_comment(format!("# {} more items", ctx.omitted));
        out.push_newline();
    }
}

fn push_array_omitted_end(ctx: &ArrayCtx, out: &mut Out<'_>) {
    if !ctx.omitted_at_start && ctx.omitted > 0 {
        out.push_indent(ctx.depth);
        out.push_comment(format!("# {} more items", ctx.omitted));
        out.push_newline();
    }
}

fn render_array_pretty(ctx: &ArrayCtx, out: &mut Out<'_>) {
    push_array_omitted_start(ctx, out);
    for (_, item) in ctx.children.iter() {
        push_yaml_array_item(out, ctx.depth, item);
    }
    push_array_omitted_end(ctx, out);
}

pub(super) fn render_array(ctx: &ArrayCtx, out: &mut Out<'_>) {
    if out.is_compact_mode() {
        super::json::render_array(ctx, out);
        return;
    }
    if ctx.children_len == 0 {
        if !ctx.inline_open {
            out.push_indent(ctx.depth);
        }
        out.push_str("[]");
        return;
    }
    render_array_pretty(ctx, out);
}

fn decode_json_string(quoted: &str) -> Option<String> {
    serde_json::from_str::<String>(quoted).ok()
}

fn needs_quotes_yaml_key(s: &str) -> bool {
    if s.is_empty() {
        return true;
    }
    let b = s.as_bytes();
    let first = b[0];
    if first.is_ascii_digit() || first == b'-' || first.is_ascii_whitespace() {
        return true;
    }
    let lower = s.to_ascii_lowercase();
    match lower.as_str() {
        "true" | "false" | "null" | "~" | "yes" | "no" | "on" | "off"
        | "y" | "n" => return true,
        _ => {}
    }
    if s.chars().last().is_some_and(char::is_whitespace) {
        return true;
    }
    for &c in b.iter() {
        match c {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b'-' => {}
            _ => return true,
        }
    }
    false
}

fn yaml_key_text_from_json_quoted(k: &str) -> String {
    if let Some(raw) = decode_json_string(k) {
        if !needs_quotes_yaml_key(&raw) {
            return raw;
        }
    }
    k.to_string()
}

fn push_object_kv(out: &mut Out<'_>, depth: usize, key_text: &str, v: &str) {
    out.push_indent(depth);
    if !has_newline(v) {
        out.push_str(&format!("{key_text}: "));
        out.push_str(v);
        out.push_newline();
    } else {
        out.push_str(&format!("{key_text}:"));
        out.push_newline();
        out.push_str(v);
        if !v.ends_with('\n') && !v.ends_with('\r') {
            out.push_newline();
        }
    }
}

fn push_object_omitted(ctx: &ObjectCtx<'_>, out: &mut Out<'_>) {
    if ctx.omitted > 0 {
        out.push_indent(ctx.depth);
        let label = if ctx.fileset_root {
            "files"
        } else {
            "properties"
        };
        out.push_comment(format!("# {} more {label}", ctx.omitted));
        out.push_newline();
    }
}

fn render_object_pretty(ctx: &ObjectCtx<'_>, out: &mut Out<'_>) {
    for (_, (k, v)) in ctx.children.iter() {
        let key_text = yaml_key_text_from_json_quoted(k);
        push_object_kv(out, ctx.depth, &key_text, v);
    }
    push_object_omitted(ctx, out);
}

pub(super) fn render_object(ctx: &ObjectCtx<'_>, out: &mut Out<'_>) {
    if out.is_compact_mode() {
        super::json::render_object(ctx, out);
        return;
    }
    if ctx.children_len == 0 {
        if !ctx.inline_open {
            out.push_indent(ctx.depth);
        }
        out.push_str("{}");
        return;
    }
    render_object_pretty(ctx, out);
}
