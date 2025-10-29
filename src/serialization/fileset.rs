use super::{RenderScope, indent};
use crate::order::ROOT_PQ_ID;

impl<'a> RenderScope<'a> {
    pub(super) fn append_js_fileset_section(
        &mut self,
        out: &mut String,
        depth: usize,
        child_pq_id: usize,
        nl: &str,
    ) {
        let raw_key =
            self.order.nodes[child_pq_id].key_in_object().unwrap_or("");
        out.push_str(&indent(depth, &self.config.indent_unit));
        out.push_str("// ");
        out.push_str(raw_key);
        out.push_str(nl);
        let rendered = self.render_node_to_string(child_pq_id, depth, false);
        out.push_str(&rendered);
        out.push(';');
        out.push_str(nl);
    }

    pub(super) fn append_js_fileset_summary(
        &self,
        out: &mut String,
        depth: usize,
        kept: usize,
        total: usize,
        nl: &str,
    ) {
        if total > kept && !nl.is_empty() {
            let blanks = if out.ends_with(nl) { 1 } else { 2 };
            for _ in 0..blanks {
                out.push_str(nl);
            }
            out.push_str(&indent(depth, &self.config.indent_unit));
            out.push_str(&format!("/* {} more files */", total - kept));
            out.push_str(nl);
        }
    }

    pub(super) fn append_pseudo_fileset_section(
        &mut self,
        out: &mut String,
        depth: usize,
        child_pq_id: usize,
        nl: &str,
    ) {
        let raw_key =
            self.order.nodes[child_pq_id].key_in_object().unwrap_or("");
        out.push_str(&indent(depth, &self.config.indent_unit));
        out.push_str("==> ");
        out.push_str(raw_key);
        out.push_str(" <==");
        out.push_str(nl);
        let rendered = self.render_node_to_string(child_pq_id, depth, false);
        out.push_str(&rendered);
    }

    pub(super) fn append_pseudo_fileset_summary(
        &self,
        out: &mut String,
        depth: usize,
        kept: usize,
        total: usize,
        nl: &str,
    ) {
        if total > kept && !nl.is_empty() {
            while out.ends_with(nl) {
                let new_len = out.len().saturating_sub(nl.len());
                out.truncate(new_len);
            }
            out.push_str(nl);
            out.push_str(nl);
            out.push_str(&indent(depth, &self.config.indent_unit));
            out.push_str(&format!("==> {} more files <==", total - kept));
        }
    }

    // Render multi-input fileset as head-style sections for JS template using
    // line comments for file names. Ensure valid JS by terminating each section
    // with a semicolon.
    pub(super) fn serialize_fileset_root_js(
        &mut self,
        depth: usize,
    ) -> String {
        let nl = &self.config.newline;
        let mut out = String::new();
        let Some(children_ids) = self.order.children.get(ROOT_PQ_ID) else {
            return out;
        };
        let kept =
            self.render_js_fileset_sections(&mut out, depth, children_ids, nl);
        let total = self
            .order
            .metrics
            .get(ROOT_PQ_ID)
            .and_then(|m| m.object_len)
            .unwrap_or(children_ids.len());
        self.append_js_fileset_summary(&mut out, depth, kept, total, nl);
        out
    }

    pub(super) fn render_js_fileset_sections(
        &mut self,
        out: &mut String,
        depth: usize,
        children_ids: &[crate::order::NodeId],
        nl: &str,
    ) -> usize {
        let mut kept = 0usize;
        for &child_id in children_ids.iter() {
            if self.inclusion_flags[child_id.0] != self.render_set_id {
                continue;
            }
            if kept > 0 {
                out.push_str(nl);
            }
            kept += 1;
            self.append_js_fileset_section(out, depth, child_id.0, nl);
        }
        kept
    }

    pub(super) fn serialize_fileset_root_pseudo(
        &mut self,
        depth: usize,
    ) -> String {
        let nl = &self.config.newline;
        let mut out = String::new();
        if let Some(children_ids) = self.order.children.get(ROOT_PQ_ID) {
            let mut kept = 0usize;
            for &child_id in children_ids.iter() {
                if self.inclusion_flags[child_id.0] != self.render_set_id {
                    continue;
                }
                if kept > 0 {
                    // ensure an empty line between files regardless of whether previous section ended with a newline
                    out.push_str(nl);
                    out.push_str(nl);
                }
                kept += 1;
                self.append_pseudo_fileset_section(
                    &mut out, depth, child_id.0, nl,
                );
            }
            let total = self
                .order
                .metrics
                .get(ROOT_PQ_ID)
                .and_then(|m| m.object_len)
                .unwrap_or(children_ids.len());
            self.append_pseudo_fileset_summary(
                &mut out, depth, kept, total, nl,
            );
        }
        out
    }
}
