use askama::Template;
use crate::OutputTemplate;

#[derive(Template)]
#[template(path = "json/array.askama", escape = "none")]
pub struct JsonArray<'a> { pub children: &'a [(usize, String)], pub children_len: usize, pub omitted: usize, pub indent0: &'a str, pub indent1: &'a str }

#[derive(Template)]
#[template(path = "json/object.askama", escape = "none")]
pub struct JsonObject<'a> { pub children: &'a [(usize, (String, String))], pub children_len: usize, pub omitted: usize, pub indent0: &'a str, pub indent1: &'a str, pub sp: &'a str }

#[derive(Template)]
#[template(path = "pseudo/array.askama", escape = "none")]
pub struct PseudoArray<'a> { pub children: &'a [(usize, String)], pub children_len: usize, pub omitted: usize, pub indent0: &'a str, pub indent1: &'a str }

#[derive(Template)]
#[template(path = "pseudo/object.askama", escape = "none")]
pub struct PseudoObject<'a> { pub children: &'a [(usize, (String, String))], pub children_len: usize, pub omitted: usize, pub indent0: &'a str, pub indent1: &'a str, pub sp: &'a str }

#[derive(Template)]
#[template(path = "js/array.askama", escape = "none")]
pub struct JsArray<'a> { pub children: &'a [(usize, String)], pub children_len: usize, pub omitted: usize, pub indent0: &'a str, pub indent1: &'a str }

#[derive(Template)]
#[template(path = "js/object.askama", escape = "none")]
pub struct JsObject<'a> { pub children: &'a [(usize, (String, String))], pub children_len: usize, pub omitted: usize, pub indent0: &'a str, pub indent1: &'a str, pub sp: &'a str }

pub struct ArrayCtx { pub children: Vec<(usize, String)>, pub children_len: usize, pub omitted: usize, pub indent0: String, pub indent1: String }
pub struct ObjectCtx { pub children: Vec<(usize, (String, String))>, pub children_len: usize, pub omitted: usize, pub indent0: String, pub indent1: String, pub sp: String }

pub fn render_array(template: OutputTemplate, ctx: &ArrayCtx) -> String {
    match template {
        OutputTemplate::Json => JsonArray { children: &ctx.children, children_len: ctx.children_len, omitted: ctx.omitted, indent0: &ctx.indent0, indent1: &ctx.indent1 }.render().unwrap(),
        OutputTemplate::Pseudo => PseudoArray { children: &ctx.children, children_len: ctx.children_len, omitted: ctx.omitted, indent0: &ctx.indent0, indent1: &ctx.indent1 }.render().unwrap(),
        OutputTemplate::Js => JsArray { children: &ctx.children, children_len: ctx.children_len, omitted: ctx.omitted, indent0: &ctx.indent0, indent1: &ctx.indent1 }.render().unwrap(),
    }
}

pub fn render_object(template: OutputTemplate, ctx: &ObjectCtx) -> String {
    match template {
        OutputTemplate::Json => JsonObject { children: &ctx.children, children_len: ctx.children_len, omitted: ctx.omitted, indent0: &ctx.indent0, indent1: &ctx.indent1, sp: &ctx.sp }.render().unwrap(),
        OutputTemplate::Pseudo => PseudoObject { children: &ctx.children, children_len: ctx.children_len, omitted: ctx.omitted, indent0: &ctx.indent0, indent1: &ctx.indent1, sp: &ctx.sp }.render().unwrap(),
        OutputTemplate::Js => JsObject { children: &ctx.children, children_len: ctx.children_len, omitted: ctx.omitted, indent0: &ctx.indent0, indent1: &ctx.indent1, sp: &ctx.sp }.render().unwrap(),
    }
}
