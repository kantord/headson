use crate::OutputTemplate;

mod core;
mod js;
mod json;
mod pseudo;
use super::color::{self};

pub struct ArrayCtx<'a> {
    pub children: Vec<(usize, String)>,
    pub children_len: usize,
    pub omitted: usize,
    pub depth: usize,
    pub indent_unit: &'a str,
    pub inline_open: bool,
    pub newline: &'a str,
    pub omitted_at_start: bool,
    pub color_enabled: bool,
}

pub struct ObjectCtx<'a> {
    pub children: Vec<(usize, (String, String))>,
    pub children_len: usize,
    pub omitted: usize,
    pub depth: usize,
    pub indent_unit: &'a str,
    pub inline_open: bool,
    pub space: &'a str,
    pub newline: &'a str,
    pub fileset_root: bool,
    pub color_enabled: bool,
}

// Color helpers facade so templates don't pass flags around.
pub trait ColorExt {
    fn omission(&self) -> &'static str;
    fn comment<S: Into<String>>(&self, body: S) -> String;
}

impl<'a> ColorExt for ArrayCtx<'a> {
    fn omission(&self) -> &'static str {
        color::omission_marker(self.color_enabled)
    }
    fn comment<S: Into<String>>(&self, body: S) -> String {
        color::color_comment(body, self.color_enabled)
    }
}

impl<'a> ColorExt for ObjectCtx<'a> {
    fn omission(&self) -> &'static str {
        color::omission_marker(self.color_enabled)
    }
    fn comment<S: Into<String>>(&self, body: S) -> String {
        color::color_comment(body, self.color_enabled)
    }
}

pub fn render_array(template: OutputTemplate, ctx: &ArrayCtx<'_>) -> String {
    match template {
        OutputTemplate::Json => json::render_array(ctx),
        OutputTemplate::Pseudo => pseudo::render_array(ctx),
        OutputTemplate::Js => js::render_array(ctx),
    }
}

pub fn render_object(template: OutputTemplate, ctx: &ObjectCtx<'_>) -> String {
    match template {
        OutputTemplate::Json => json::render_object(ctx),
        OutputTemplate::Pseudo => pseudo::render_object(ctx),
        OutputTemplate::Js => js::render_object(ctx),
    }
}
