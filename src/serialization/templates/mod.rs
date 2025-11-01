use crate::OutputTemplate;
use crate::order::NodeKind;
use crate::serialization::output::Out;

mod core;
mod js;
mod json;
mod pseudo;
mod text;
mod yaml;
//
pub struct ArrayCtx {
    pub children: Vec<(usize, (NodeKind, String))>,
    pub children_len: usize,
    pub omitted: usize,
    pub depth: usize,
    pub inline_open: bool,
    pub omitted_at_start: bool,
}

pub struct ObjectCtx<'a> {
    pub children: Vec<(usize, (String, String))>,
    pub children_len: usize,
    pub omitted: usize,
    pub depth: usize,
    pub inline_open: bool,
    pub space: &'a str,
    pub fileset_root: bool,
}

// Color helpers facade so templates don't pass flags around.
// Color helpers are now provided via the Out writer in super::output.

pub fn render_array(
    template: OutputTemplate,
    ctx: &ArrayCtx,
    out: &mut Out<'_>,
) {
    match template {
        OutputTemplate::Json => json::render_array(ctx, out),
        OutputTemplate::Auto | OutputTemplate::Pseudo => {
            pseudo::render_array(ctx, out)
        }
        OutputTemplate::Js => js::render_array(ctx, out),
        OutputTemplate::Yaml => yaml::render_array(ctx, out),
        OutputTemplate::Text => text::render_array(ctx, out),
    }
}

pub fn render_object(
    template: OutputTemplate,
    ctx: &ObjectCtx<'_>,
    out: &mut Out<'_>,
) {
    match template {
        OutputTemplate::Json => json::render_object(ctx, out),
        OutputTemplate::Auto | OutputTemplate::Pseudo => {
            pseudo::render_object(ctx, out)
        }
        OutputTemplate::Js => js::render_object(ctx, out),
        OutputTemplate::Yaml => yaml::render_object(ctx, out),
        OutputTemplate::Text => text::render_object(ctx, out),
    }
}
