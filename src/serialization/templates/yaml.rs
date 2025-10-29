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
        push_yaml_scalar(out, item.trim());
        out.push_newline();
        return;
    }
    // Multi-line item: print first logical line after "- ", and align
    // all following lines under the first character after the dash.
    let mut iter = item.lines();
    if let Some(first) = iter.next() {
        out.push_indent(depth);
        out.push_str("- ");
        out.push_str(first.trim_start());
        out.push_newline();
    }
    for rest in iter {
        out.push_indent(depth);
        out.push_str("  ");
        out.push_str(rest.trim_start());
        out.push_newline();
    }
}

fn push_yaml_scalar(out: &mut Out<'_>, token: &str) {
    // If it's a JSON string literal, decode, then decide quoting and color.
    if let Some(raw) = decode_json_string(token) {
        if !needs_quotes_yaml_value(&raw) {
            // Unquoted YAML string value: color the raw text via Out.
            out.push_string_unquoted(&raw);
            return;
        }
        // Quoted string: reuse JSON quoting and let Out color the literal (including quotes).
        out.push_string_literal(token);
        return;
    }
    // Non-string token (number, bool, null) → pass through unchanged.
    out.push_str(token);
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
    for (_, (_, item)) in ctx.children.iter() {
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
    // Color the key according to role; colon and space remain uncolored.
    out.push_key(key_text);
    if !has_newline(v) {
        out.push_str(": ");
        push_yaml_scalar(out, v);
        out.push_newline();
    } else {
        // Multiline value: print key and start block on next line.
        out.push_str(":");
        out.push_newline();
        out.push_str(v);
        if !v.ends_with('\n') && !v.ends_with('\r') {
            out.push_newline();
        }
    }
}

fn yaml_value_has_linebreaks(s: &str) -> bool {
    s.contains('\n') || s.contains('\r')
}

fn yaml_value_has_outer_ws(s: &str) -> bool {
    s.starts_with(char::is_whitespace) || s.ends_with(char::is_whitespace)
}

fn yaml_value_is_reserved(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "true"
            | "false"
            | "null"
            | "~"
            | "yes"
            | "no"
            | "on"
            | "off"
            | "y"
            | "n"
    )
}

fn yaml_value_looks_numeric(s: &str) -> bool {
    let bytes = s.as_bytes();
    match bytes.first().copied() {
        Some(b'-' | b'+' | b'0'..=b'9') => {
            let mut has_digit = false;
            for &c in bytes {
                match c {
                    b'0'..=b'9' => has_digit = true,
                    b'.' | b'e' | b'E' | b'+' | b'-' => {}
                    _ => return false,
                }
            }
            has_digit
        }
        _ => false,
    }
}

fn yaml_value_has_disallowed_punct(s: &str) -> bool {
    const DISALLOWED: [char; 15] = [
        ':', '#', '{', '}', '[', ']', ',', '&', '*', '?', '|', '>', '@', '%',
        '!',
    ];
    s.chars().any(|c| DISALLOWED.contains(&c))
}

fn needs_quotes_yaml_value(s: &str) -> bool {
    s.is_empty()
        || yaml_value_has_linebreaks(s)
        || yaml_value_has_outer_ws(s)
        || yaml_value_is_reserved(s)
        || yaml_value_looks_numeric(s)
        || yaml_value_has_disallowed_punct(s)
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
