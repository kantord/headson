use crate::OutputTemplate;

pub mod js;
pub mod json;
pub mod pseudo;

pub struct ArrayCtx {
    pub children: Vec<(usize, String)>,
    pub children_len: usize,
    pub omitted: usize,
    pub indent0: String,
    pub indent1: String,
}

pub struct ObjectCtx {
    pub children: Vec<(usize, (String, String))>,
    pub children_len: usize,
    pub omitted: usize,
    pub indent0: String,
    pub indent1: String,
    pub sp: String,
}

pub fn render_array(template: OutputTemplate, ctx: &ArrayCtx) -> String {
    match template {
        OutputTemplate::Json => json::render_array(ctx),
        OutputTemplate::Pseudo => pseudo::render_array(ctx),
        OutputTemplate::Js => js::render_array(ctx),
    }
}

pub fn render_object(template: OutputTemplate, ctx: &ObjectCtx) -> String {
    match template {
        OutputTemplate::Json => json::render_object(ctx),
        OutputTemplate::Pseudo => pseudo::render_object(ctx),
        OutputTemplate::Js => js::render_object(ctx),
    }
}
