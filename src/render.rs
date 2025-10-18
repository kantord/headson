use askama::Template;
use crate::OutputTemplate;

#[derive(Template)]
#[template(path = "json_array.askama", escape = "none")]
pub struct JsonArray<'a> { pub children: &'a [(usize, String)], pub children_len: usize, pub omitted: usize }

#[derive(Template)]
#[template(path = "json_object.askama", escape = "none")]
pub struct JsonObject<'a> { pub children: &'a [(usize, (String, String))], pub children_len: usize, pub omitted: usize }

#[derive(Template)]
#[template(path = "pseudo_array.askama", escape = "none")]
pub struct PseudoArray<'a> { pub children: &'a [(usize, String)], pub children_len: usize, pub omitted: usize }

#[derive(Template)]
#[template(path = "pseudo_object.askama", escape = "none")]
pub struct PseudoObject<'a> { pub children: &'a [(usize, (String, String))], pub children_len: usize, pub omitted: usize }

#[derive(Template)]
#[template(path = "js_array.askama", escape = "none")]
pub struct JsArray<'a> { pub children: &'a [(usize, String)], pub children_len: usize, pub omitted: usize }

#[derive(Template)]
#[template(path = "js_object.askama", escape = "none")]
pub struct JsObject<'a> { pub children: &'a [(usize, (String, String))], pub children_len: usize, pub omitted: usize }

pub struct ArrayCtx { pub children: Vec<(usize, String)>, pub children_len: usize, pub omitted: usize }
pub struct ObjectCtx { pub children: Vec<(usize, (String, String))>, pub children_len: usize, pub omitted: usize }

pub fn render_array(template: OutputTemplate, ctx: &ArrayCtx) -> String {
    match template {
        OutputTemplate::Json => JsonArray { children: &ctx.children, children_len: ctx.children_len, omitted: ctx.omitted }.render().unwrap(),
        OutputTemplate::Pseudo => PseudoArray { children: &ctx.children, children_len: ctx.children_len, omitted: ctx.omitted }.render().unwrap(),
        OutputTemplate::Js => JsArray { children: &ctx.children, children_len: ctx.children_len, omitted: ctx.omitted }.render().unwrap(),
    }
}

pub fn render_object(template: OutputTemplate, ctx: &ObjectCtx) -> String {
    match template {
        OutputTemplate::Json => JsonObject { children: &ctx.children, children_len: ctx.children_len, omitted: ctx.omitted }.render().unwrap(),
        OutputTemplate::Pseudo => PseudoObject { children: &ctx.children, children_len: ctx.children_len, omitted: ctx.omitted }.render().unwrap(),
        OutputTemplate::Js => JsObject { children: &ctx.children, children_len: ctx.children_len, omitted: ctx.omitted }.render().unwrap(),
    }
}
