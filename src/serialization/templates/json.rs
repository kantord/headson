use super::core::{Style, render_array_with, render_object_with};
use super::{ArrayCtx, ObjectCtx};

struct Json;

impl Style for Json {
    fn array_empty(open_indent: &str, _ctx: &ArrayCtx) -> String {
        format!("{open_indent}[]")
    }

    fn object_empty(open_indent: &str, _ctx: &ObjectCtx) -> String {
        format!("{open_indent}{{}}")
    }
}

pub fn render_array(ctx: &ArrayCtx) -> String {
    render_array_with::<Json>(ctx)
}

pub fn render_object(ctx: &ObjectCtx) -> String {
    render_object_with::<Json>(ctx)
}
