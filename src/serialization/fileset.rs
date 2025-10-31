use super::RenderScope;
use crate::format::Format;
use crate::order::{ObjectType, ROOT_PQ_ID};
use crate::serialization::types::OutputTemplate;

impl<'a> RenderScope<'a> {
    pub(super) fn try_render_fileset_root(
        &mut self,
        id: usize,
        depth: usize,
    ) -> Option<String> {
        if id == ROOT_PQ_ID
            && self.order.object_type.get(id) == Some(&ObjectType::Fileset)
            && !self.config.newline.is_empty()
        {
            return Some(self.render_fileset_sections(depth));
        }
        None
    }

    #[allow(
        clippy::cognitive_complexity,
        reason = "Straight-line rendering with early-continues; splitting would harm locality."
    )]
    fn render_fileset_sections(&mut self, depth: usize) -> String {
        let nl = &self.config.newline;
        let mut out = String::new();
        let Some(children_ids) = self.order.children.get(ROOT_PQ_ID) else {
            return out;
        };
        let mut kept = 0usize;
        for &child_id in children_ids.iter() {
            if self.inclusion_flags[child_id.0] != self.render_set_id {
                continue;
            }
            if kept > 0 {
                out.push_str(nl);
                out.push_str(nl);
            }
            kept += 1;
            let raw_key =
                self.order.nodes[child_id.0].key_in_object().unwrap_or("");
            let indent = self.config.indent_unit.repeat(depth);
            out.push_str(&indent);
            out.push_str("==> ");
            out.push_str(raw_key);
            out.push_str(" <==");
            out.push_str(nl);

            // In Auto mode select per-file template by extension; otherwise
            // honor the user-chosen template globally.
            let rendered =
                if matches!(self.config.template, OutputTemplate::Auto) {
                    let fmt = Format::from_filename(raw_key);
                    let template = match fmt {
                        Format::Json => OutputTemplate::Json,
                        Format::Yaml => OutputTemplate::Yaml,
                        Format::Unknown => OutputTemplate::Pseudo,
                    };
                    self.render_node_to_string_with_template(
                        child_id.0, depth, false, template,
                    )
                } else {
                    self.render_node_to_string(child_id.0, depth, false)
                };
            out.push_str(&rendered);
        }
        let total = self
            .order
            .metrics
            .get(ROOT_PQ_ID)
            .and_then(|m| m.object_len)
            .unwrap_or(children_ids.len());
        if total > kept && !nl.is_empty() {
            // Ensure a clear visual break before summary
            out.push_str(nl);
            out.push_str(nl);
            let indent = self.config.indent_unit.repeat(depth);
            out.push_str(&indent);
            out.push_str(&format!("==> {} more files <==", total - kept));
        }
        out
    }
}
