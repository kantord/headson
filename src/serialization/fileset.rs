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

    fn render_fileset_sections(&mut self, depth: usize) -> String {
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
                self.fileset_push_section_gap(&mut out);
            }
            kept += 1;
            let raw_key =
                self.order.nodes[child_id.0].key_in_object().unwrap_or("");
            out.push_str(&self.fileset_header_line(depth, raw_key));
            let rendered =
                self.fileset_render_child(child_id.0, depth, raw_key);
            out.push_str(&rendered);
        }
        let total = self
            .order
            .metrics
            .get(ROOT_PQ_ID)
            .and_then(|m| m.object_len)
            .unwrap_or(children_ids.len());
        if total > kept && !self.config.newline.is_empty() {
            self.fileset_push_section_gap(&mut out);
            out.push_str(&self.fileset_summary_line(depth, total - kept));
        }
        out
    }

    fn fileset_push_section_gap(&self, out: &mut String) {
        let nl = &self.config.newline;
        out.push_str(nl);
        out.push_str(nl);
    }

    fn fileset_header_line(&self, depth: usize, key: &str) -> String {
        let nl = &self.config.newline;
        let indent = self.config.indent_unit.repeat(depth);
        let mut s = String::with_capacity(indent.len() + key.len() + 8);
        s.push_str(&indent);
        s.push_str("==> ");
        s.push_str(key);
        s.push_str(" <==");
        s.push_str(nl);
        s
    }

    fn fileset_summary_line(&self, depth: usize, omitted: usize) -> String {
        let indent = self.config.indent_unit.repeat(depth);
        format!("{indent}==> {omitted} more files <==")
    }

    fn fileset_render_child(
        &mut self,
        child_id: usize,
        depth: usize,
        raw_key: &str,
    ) -> String {
        if matches!(self.config.template, OutputTemplate::Auto) {
            let fmt = Format::from_filename(raw_key);
            let template = fmt.to_output_template(OutputTemplate::Pseudo);
            return self.render_node_to_string_with_template(
                child_id, depth, false, template,
            );
        }
        self.render_node_to_string(child_id, depth, false)
    }
}
