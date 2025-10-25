use crate::order::ObjectType;
use crate::order::{NodeKind, PriorityOrder, ROOT_PQ_ID};
pub mod templates;
pub mod types;
use self::templates::{ArrayCtx, ObjectCtx, render_array, render_object};

fn indent(depth: usize, unit: &str) -> String {
    unit.repeat(depth)
}

type ArrayChildPair = (usize, String);
type ObjectChildPair = (usize, (String, String));

pub(crate) struct RenderScope<'a> {
    // Priority-ordered view of the parsed JSON tree.
    order: &'a PriorityOrder,
    // Per-node inclusion flag: a node is included in the current render attempt
    // when inclusion_flags[node_id] == render_set_id. This avoids clearing the
    // vector between render attempts by bumping render_set_id each time.
    inclusion_flags: &'a [u32],
    // Identifier for the current inclusion set (render pass).
    render_set_id: u32,
    // Rendering configuration (template, whitespace, etc.).
    config: &'a crate::RenderConfig,
}

impl<'a> RenderScope<'a> {
    fn render_has_newline(&self, s: &str) -> bool {
        let nl = &self.config.newline;
        if nl.is_empty() {
            return false;
        }
        if nl == "\n" {
            return s.as_bytes().contains(&b'\n');
        }
        s.contains(nl)
    }

    fn push_array_child_line(
        &self,
        out: &mut Vec<ArrayChildPair>,
        index: usize,
        child_kind: NodeKind,
        depth: usize,
        rendered: String,
    ) {
        if self.render_has_newline(&rendered) {
            out.push((index, rendered));
            return;
        }
        match child_kind {
            NodeKind::Array | NodeKind::Object => {
                out.push((index, rendered));
            }
            _ => {
                let child_indent = indent(depth + 1, &self.config.indent_unit);
                out.push((index, format!("{child_indent}{rendered}")));
            }
        }
    }
    fn append_js_fileset_section(
        &mut self,
        out: &mut String,
        depth: usize,
        child_pq_id: usize,
        nl: &str,
    ) {
        let raw_key = self.order.nodes[child_pq_id]
            .key_in_object
            .as_deref()
            .unwrap_or("");
        out.push_str(&indent(depth, &self.config.indent_unit));
        out.push_str("// ");
        out.push_str(raw_key);
        out.push_str(nl);
        let rendered = self.serialize_node(child_pq_id, depth, false);
        out.push_str(&rendered);
        out.push(';');
        out.push_str(nl);
    }

    fn append_js_fileset_summary(
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

    fn append_pseudo_fileset_section(
        &mut self,
        out: &mut String,
        depth: usize,
        child_pq_id: usize,
        nl: &str,
    ) {
        let raw_key = self.order.nodes[child_pq_id]
            .key_in_object
            .as_deref()
            .unwrap_or("");
        out.push_str(&indent(depth, &self.config.indent_unit));
        out.push_str("==> ");
        out.push_str(raw_key);
        out.push_str(" <==");
        out.push_str(nl);
        let rendered = self.serialize_node(child_pq_id, depth, false);
        out.push_str(&rendered);
    }

    fn append_pseudo_fileset_summary(
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
    fn count_kept_children(&self, id: usize) -> usize {
        if let Some(kids) = self.order.children.get(id) {
            let mut kept = 0usize;
            for &cid in kids {
                if self.inclusion_flags[cid.0] == self.render_set_id {
                    kept += 1;
                }
            }
            kept
        } else {
            0
        }
    }

    fn omitted_for_string(&self, id: usize, kept: usize) -> Option<usize> {
        let m = &self.order.metrics[id];
        if let Some(orig) = m.string_len {
            if orig > kept {
                return Some(orig - kept);
            }
            if m.string_truncated {
                return Some(1);
            }
            None
        } else if m.string_truncated {
            Some(1)
        } else {
            None
        }
    }

    fn omitted_for(
        &self,
        id: usize,
        kind: NodeKind,
        kept: usize,
    ) -> Option<usize> {
        match kind {
            NodeKind::Array => {
                self.order.metrics[id].array_len.and_then(|orig| {
                    if orig > kept { Some(orig - kept) } else { None }
                })
            }
            NodeKind::String => self.omitted_for_string(id, kept),
            NodeKind::Object => {
                self.order.metrics[id].object_len.and_then(|orig| {
                    if orig > kept { Some(orig - kept) } else { None }
                })
            }
            _ => None,
        }
    }

    fn serialize_array(
        &mut self,
        id: usize,
        depth: usize,
        inline: bool,
    ) -> String {
        let config = self.config;
        let (children_pairs, kept) = self.gather_array_children(id, depth);
        let node = &self.order.nodes[id];
        let omitted = self.omitted_for(id, node.kind, kept).unwrap_or(0);
        if kept == 0 && omitted == 0 {
            return "[]".to_string();
        }
        let ctx = ArrayCtx {
            children: children_pairs,
            children_len: kept,
            omitted,
            depth,
            indent_unit: &config.indent_unit,
            inline_open: inline,
            newline: &config.newline,
            omitted_at_start: config.prefer_tail_arrays,
        };
        render_array(config.template, &ctx)
    }

    fn serialize_object(
        &mut self,
        id: usize,
        depth: usize,
        inline: bool,
    ) -> String {
        let config = self.config;
        // Special-case: fileset root in Pseudo/JS templates → head-style sections
        if id == ROOT_PQ_ID
            && self.order.object_type.get(id) == Some(&ObjectType::Fileset)
            && !config.newline.is_empty()
        {
            match config.template {
                crate::OutputTemplate::Pseudo => {
                    return self.serialize_fileset_root_pseudo(depth);
                }
                crate::OutputTemplate::Js => {
                    return self.serialize_fileset_root_js(depth);
                }
                _ => {}
            }
        }
        let (children_pairs, kept) = self.gather_object_children(id, depth);
        let node = &self.order.nodes[id];
        let omitted = self.omitted_for(id, node.kind, kept).unwrap_or(0);
        if kept == 0 && omitted == 0 {
            return "{}".to_string();
        }
        let ctx = ObjectCtx {
            children: children_pairs,
            children_len: kept,
            omitted,
            depth,
            indent_unit: &config.indent_unit,
            inline_open: inline,
            space: &config.space,
            newline: &config.newline,
            fileset_root: id == ROOT_PQ_ID
                && self.order.object_type.get(id)
                    == Some(&ObjectType::Fileset),
        };
        render_object(config.template, &ctx)
    }

    fn serialize_string(&mut self, id: usize) -> String {
        let kept = self.count_kept_children(id);
        let node = &self.order.nodes[id];
        let omitted = self.omitted_for(id, node.kind, kept).unwrap_or(0);
        let full: &str = node.string_value.as_deref().unwrap_or("");
        if omitted == 0 {
            return crate::utils::json::json_string(full);
        }
        let prefix = crate::utils::text::take_n_graphemes(full, kept);
        let truncated = format!("{prefix}…");
        crate::utils::json::json_string(&truncated)
    }

    fn serialize_number(&self, id: usize) -> String {
        let it = &self.order.nodes[id];
        if let Some(n) = it.number_value.as_ref() {
            if let Some(i) = n.as_i64() {
                return i.to_string();
            }
            if let Some(u) = n.as_u64() {
                return u.to_string();
            }
            if n.as_f64().is_some() {
                return n.to_string();
            }
        }
        "0".to_string()
    }

    fn serialize_bool(&self, id: usize) -> String {
        let it = &self.order.nodes[id];
        match it.bool_value {
            Some(true) => "true".to_string(),
            Some(false) | None => "false".to_string(),
        }
    }

    fn serialize_node(
        &mut self,
        id: usize,
        depth: usize,
        inline: bool,
    ) -> String {
        let it = &self.order.nodes[id];
        match it.kind {
            NodeKind::Array => self.serialize_array(id, depth, inline),
            NodeKind::Object => self.serialize_object(id, depth, inline),
            NodeKind::String => self.serialize_string(id),
            NodeKind::Number => self.serialize_number(id),
            NodeKind::Bool => self.serialize_bool(id),
            NodeKind::Null => "null".to_string(),
        }
    }

    fn gather_array_children(
        &mut self,
        id: usize,
        depth: usize,
    ) -> (Vec<ArrayChildPair>, usize) {
        let mut children_pairs: Vec<ArrayChildPair> = Vec::new();
        let mut kept = 0usize;
        if let Some(children_ids) = self.order.children.get(id) {
            for (i, &child_id) in children_ids.iter().enumerate() {
                if self.inclusion_flags[child_id.0] != self.render_set_id {
                    continue;
                }
                kept += 1;
                let child_kind = self.order.nodes[child_id.0].kind;
                let rendered =
                    self.serialize_node(child_id.0, depth + 1, false);
                self.push_array_child_line(
                    &mut children_pairs,
                    i,
                    child_kind,
                    depth,
                    rendered,
                );
            }
        }
        (children_pairs, kept)
    }

    fn gather_object_children(
        &mut self,
        id: usize,
        depth: usize,
    ) -> (Vec<ObjectChildPair>, usize) {
        let mut children_pairs: Vec<ObjectChildPair> = Vec::new();
        let mut kept = 0usize;
        if let Some(children_ids) = self.order.children.get(id) {
            for (i, &child_id) in children_ids.iter().enumerate() {
                if self.inclusion_flags[child_id.0] != self.render_set_id {
                    continue;
                }
                kept += 1;
                let child = &self.order.nodes[child_id.0];
                let raw_key = child.key_in_object.as_deref().unwrap_or("");
                let key = crate::utils::json::json_string(raw_key);
                let val = self.serialize_node(child_id.0, depth + 1, true);
                children_pairs.push((i, (key, val)));
            }
        }
        (children_pairs, kept)
    }

    // Render multi-input fileset as head-style sections for Pseudo template.
    fn serialize_fileset_root_pseudo(&mut self, depth: usize) -> String {
        let nl = &self.config.newline;
        let mut out = String::new();
        if let Some(children_ids) = self.order.children.get(ROOT_PQ_ID) {
            let mut kept = 0usize;
            for &child_id in children_ids.iter() {
                if self.inclusion_flags[child_id.0] != self.render_set_id {
                    continue;
                }
                if kept > 0 {
                    // ensure an empty line between files regardless of whether
                    // previous section ended with a newline
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

    // Render multi-input fileset as head-style sections for JS template using
    // line comments for file names. Ensure valid JS by terminating each section
    // with a semicolon.
    fn serialize_fileset_root_js(&mut self, depth: usize) -> String {
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

    fn render_js_fileset_sections(
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
}

/// Prepare a render set by including the first `top_k` nodes by priority
/// and all of their ancestors so the output remains structurally valid.
pub fn prepare_render_set_top_k_and_ancestors(
    order_build: &PriorityOrder,
    top_k: usize,
    inclusion_flags: &mut Vec<u32>,
    render_id: u32,
) {
    if inclusion_flags.len() < order_build.total_nodes {
        inclusion_flags.resize(order_build.total_nodes, 0);
    }
    let k = top_k.min(order_build.total_nodes);
    crate::utils::graph::mark_top_k_and_ancestors(
        order_build,
        k,
        inclusion_flags,
        render_id,
    );
}

/// Render using a previously prepared render set (inclusion flags matching `render_id`).
pub fn render_from_render_set(
    order_build: &PriorityOrder,
    inclusion_flags: &[u32],
    render_id: u32,
    config: &crate::RenderConfig,
) -> String {
    // Root PQ id is a fixed invariant (0).
    let root_id = ROOT_PQ_ID;
    let mut scope = RenderScope {
        order: order_build,
        inclusion_flags,
        render_set_id: render_id,
        config,
    };
    scope.serialize_node(root_id, 0, false)
}

/// Convenience: prepare the render set for `top_k` nodes and render in one call.
pub fn render_top_k(
    order_build: &PriorityOrder,
    top_k: usize,
    inclusion_flags: &mut Vec<u32>,
    render_id: u32,
    config: &crate::RenderConfig,
) -> String {
    prepare_render_set_top_k_and_ancestors(
        order_build,
        top_k,
        inclusion_flags,
        render_id,
    );
    render_from_render_set(order_build, inclusion_flags, render_id, config)
}

//

#[cfg(test)]
mod tests {
    use super::*;
    use crate::order::build_order;
    use insta::assert_snapshot;

    #[test]
    fn arena_render_empty_array() {
        let arena = crate::json_ingest::build_json_tree_arena(
            "[]",
            &crate::PriorityConfig::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        let build = build_order(
            &arena,
            &crate::PriorityConfig::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        let mut marks = vec![0u32; build.total_nodes];
        let out = render_top_k(
            &build,
            10,
            &mut marks,
            1,
            &crate::RenderConfig {
                template: crate::OutputTemplate::Json,
                indent_unit: "  ".to_string(),
                space: " ".to_string(),
                newline: "\n".to_string(),
                prefer_tail_arrays: false,
            },
        );
        assert_snapshot!("arena_render_empty", out);
    }

    #[test]
    fn newline_detection_crlf_array_child() {
        // Ensure we exercise the render_has_newline branch that checks
        // arbitrary newline sequences (e.g., "\r\n") via s.contains(nl).
        let arena = crate::json_ingest::build_json_tree_arena(
            "[{\"a\":1,\"b\":2}]",
            &crate::PriorityConfig::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        let build = build_order(
            &arena,
            &crate::PriorityConfig::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        let mut marks = vec![0u32; build.total_nodes];
        let out = render_top_k(
            &build,
            usize::MAX,
            &mut marks,
            1,
            &crate::RenderConfig {
                template: crate::OutputTemplate::Json,
                indent_unit: "  ".to_string(),
                space: " ".to_string(),
                // Use CRLF to force the contains(nl) path.
                newline: "\r\n".to_string(),
                prefer_tail_arrays: false,
            },
        );
        // Sanity: output should contain CRLF newlines and render the object child across lines.
        assert!(
            out.contains("\r\n"),
            "expected CRLF newlines in output: {out:?}"
        );
        assert!(out.starts_with("["));
    }

    #[test]
    fn arena_render_single_string_array() {
        let arena = crate::json_ingest::build_json_tree_arena(
            "[\"ab\"]",
            &crate::PriorityConfig::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        let build = build_order(
            &arena,
            &crate::PriorityConfig::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        let mut marks = vec![0u32; build.total_nodes];
        let out = render_top_k(
            &build,
            10,
            &mut marks,
            1,
            &crate::RenderConfig {
                template: crate::OutputTemplate::Json,
                indent_unit: "  ".to_string(),
                space: " ".to_string(),
                newline: "\n".to_string(),
                prefer_tail_arrays: false,
            },
        );
        assert_snapshot!("arena_render_single", out);
    }

    #[test]
    fn arena_render_object_partial_js() {
        // Object with three properties; render top_k small so only one child is kept.
        let arena = crate::json_ingest::build_json_tree_arena(
            "{\"a\":1,\"b\":2,\"c\":3}",
            &crate::PriorityConfig::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        let build = build_order(
            &arena,
            &crate::PriorityConfig::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        let mut flags = vec![0u32; build.total_nodes];
        // top_k=2 → root object + first property
        let out = render_top_k(
            &build,
            2,
            &mut flags,
            1,
            &crate::RenderConfig {
                template: crate::OutputTemplate::Js,
                indent_unit: "  ".to_string(),
                space: " ".to_string(),
                newline: "\n".to_string(),
                prefer_tail_arrays: false,
            },
        );
        // Should be a valid JS object with one property and an omitted summary.
        assert!(out.starts_with("{\n"));
        assert!(
            out.contains("/* 2 more properties */"),
            "missing omitted summary: {out:?}"
        );
        assert!(
            out.contains("\"a\": 1")
                || out.contains("\"b\": 2")
                || out.contains("\"c\": 3")
        );
    }
}
