use super::core::{Style, render_array_with, render_object_with};
use super::{ArrayCtx, ObjectCtx};

struct Json;

impl Style for Json {
    fn array_empty(open_indent: &str, _ctx: &ArrayCtx<'_>) -> String {
        format!("{open_indent}[]")
    }

    fn object_empty(open_indent: &str, _ctx: &ObjectCtx<'_>) -> String {
        format!("{open_indent}{{}}")
    }
}

pub(super) fn render_array(ctx: &ArrayCtx<'_>) -> String {
    render_array_with::<Json>(ctx)
}

pub(super) fn render_object(ctx: &ObjectCtx<'_>) -> String {
    render_object_with::<Json>(ctx)
}
