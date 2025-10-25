use crate::OutputTemplate;

mod core;
mod js;
mod json;
mod pseudo;

pub struct ArrayCtx<'a> {
    pub children: Vec<(usize, String)>,
    pub children_len: usize,
    pub omitted: usize,
    pub depth: usize,
    pub indent_unit: &'a str,
    pub inline_open: bool,
    pub newline: &'a str,
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
