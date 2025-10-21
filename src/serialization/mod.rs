use crate::order::{NodeKind, PriorityOrder, ROOT_PQ_ID};
pub mod templates;
pub mod types;
use self::templates::{ArrayCtx, ObjectCtx, render_array, render_object};

fn indent(depth: usize, unit: &str) -> String {
    unit.repeat(depth)
}

// No longer needed: indentation is handled by renderers with an `inline` flag

type ArrayChildPair = (usize, String);
type ObjectChildPair = (usize, (String, String));

// Rendering scope extracted from the top-level function to reduce function length
pub(crate) struct RenderScope<'a> {
    pq: &'a PriorityOrder,
    marks: &'a [u32],
    mark_gen: u32,
    config: &'a crate::RenderConfig,
    nodes_built: usize,
    max_depth: usize,
}

impl<'a> RenderScope<'a> {
    fn count_kept_children(&self, id: usize) -> usize {
        if let Some(kids) = self.pq.children_of.get(id) {
            let mut kept = 0usize;
            for &cid in kids {
                if self.marks[cid] == self.mark_gen {
                    kept += 1;
                }
            }
            kept
        } else {
            0
        }
    }

    fn omitted_for_string(&self, id: usize, kept: usize) -> Option<usize> {
        let m = &self.pq.metrics[id];
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
        kind: &NodeKind,
        kept: usize,
    ) -> Option<usize> {
        match kind {
            NodeKind::Array => {
                self.pq.metrics[id].array_len.and_then(|orig| {
                    if orig > kept { Some(orig - kept) } else { None }
                })
            }
            NodeKind::String => self.omitted_for_string(id, kept),
            NodeKind::Object => {
                self.pq.metrics[id].object_len.and_then(|orig| {
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
        let node = &self.pq.id_to_item[id];
        let omitted = self.omitted_for(id, &node.kind, kept).unwrap_or(0);
        if kept == 0 && omitted == 0 {
            return "[]".to_string();
        }
        let ctx = ArrayCtx {
            children: children_pairs,
            children_len: kept,
            omitted,
            depth,
            indent_unit: config.indent_unit.clone(),
            inline_open: inline,
            newline: config.newline.clone(),
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
        let (children_pairs, kept) = self.gather_object_children(id, depth);
        let node = &self.pq.id_to_item[id];
        let omitted = self.omitted_for(id, &node.kind, kept).unwrap_or(0);
        if kept == 0 && omitted == 0 {
            return "{}".to_string();
        }
        let ctx = ObjectCtx {
            children: children_pairs,
            children_len: kept,
            omitted,
            depth,
            indent_unit: config.indent_unit.clone(),
            inline_open: inline,
            space: config.space.clone(),
            newline: config.newline.clone(),
        };
        render_object(config.template, &ctx)
    }

    fn serialize_string(&mut self, id: usize) -> String {
        let kept = self.count_kept_children(id);
        let node = &self.pq.id_to_item[id];
        let omitted = self.omitted_for(id, &node.kind, kept).unwrap_or(0);
        let full = node.string_value.clone().unwrap_or_default();
        if omitted == 0 {
            return crate::utils::json::json_string(&full);
        }
        let prefix = crate::utils::text::take_n_graphemes(full.as_str(), kept);
        let truncated = format!("{prefix}…");
        crate::utils::json::json_string(&truncated)
    }

    fn serialize_number(&self, id: usize) -> String {
        let it = &self.pq.id_to_item[id];
        if let Some(n) = it.number_value.as_ref() {
            if let Some(i) = n.as_i64() {
                return i.to_string();
            }
            if let Some(u) = n.as_u64() {
                return u.to_string();
            }
            if let Some(f) = n.as_f64() {
                return if f == 0.0 {
                    "0.0".to_string()
                } else {
                    n.to_string()
                };
            }
        }
        "0".to_string()
    }

    fn serialize_bool(&self, id: usize) -> String {
        let it = &self.pq.id_to_item[id];
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
        self.nodes_built += 1;
        if depth > self.max_depth {
            self.max_depth = depth;
        }
        let it = &self.pq.id_to_item[id];
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
        let config = self.config;
        let mut children_pairs: Vec<ArrayChildPair> = Vec::new();
        let mut kept = 0usize;
        if let Some(children_ids) = self.pq.children_of.get(id) {
            for (i, &child_id) in children_ids.iter().enumerate() {
                if self.marks[child_id] != self.mark_gen {
                    continue;
                }
                kept += 1;
                let rendered = self.serialize_node(child_id, depth + 1, false);
                let child_indent = indent(depth + 1, &config.indent_unit);
                if !config.newline.is_empty()
                    && rendered.contains(&config.newline)
                {
                    children_pairs.push((i, rendered));
                } else {
                    children_pairs
                        .push((i, format!("{child_indent}{rendered}")));
                }
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
        if let Some(children_ids) = self.pq.children_of.get(id) {
            for (i, &child_id) in children_ids.iter().enumerate() {
                if self.marks[child_id] != self.mark_gen {
                    continue;
                }
                kept += 1;
                let child = &self.pq.id_to_item[child_id];
                let raw_key = child.key_in_object.clone().unwrap_or_default();
                let key = crate::utils::json::json_string(&raw_key);
                let val = self.serialize_node(child_id, depth + 1, true);
                children_pairs.push((i, (key, val)));
            }
        }
        (children_pairs, kept)
    }
}

// Helper: mark first k nodes by order and their ancestors
// Ancestor marking moved to utils::graph

/// Render a budget-limited preview directly from the arena using inclusion marks.
pub fn render_arena_with_marks(
    order_build: &PriorityOrder,
    budget: usize,
    marks: &mut Vec<u32>,
    mark_gen: u32,
    config: &crate::RenderConfig,
) -> String {
    if marks.len() < order_build.total_nodes {
        marks.resize(order_build.total_nodes, 0);
    }
    // Phase 1: Mark the first `k` nodes (ids_by_order[..k]) and all their ancestors
    let k = budget.min(order_build.total_nodes);
    crate::utils::graph::mark_top_k_and_ancestors(
        order_build,
        k,
        marks,
        mark_gen,
    );

    // Root PQ id is a fixed invariant (0).
    let root_id = ROOT_PQ_ID;
    let mut scope = RenderScope {
        pq: order_build,
        marks,
        mark_gen,
        config,
        nodes_built: 0,
        max_depth: 0,
    };
    scope.serialize_node(root_id, 0, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_snapshot;

    #[test]
    fn arena_render_empty_array() {
        let arena = crate::stream_arena::build_stream_arena(
            "[]",
            &crate::PriorityConfig::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        let build = crate::build_priority_order_from_arena(
            &arena,
            &crate::PriorityConfig::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        let mut marks = vec![0u32; build.total_nodes];
        let out = render_arena_with_marks(
            &build,
            10,
            &mut marks,
            1,
            &crate::RenderConfig {
                template: crate::OutputTemplate::Json,
                indent_unit: "  ".to_string(),
                space: " ".to_string(),
                newline: "\n".to_string(),
            },
        );
        assert_snapshot!("arena_render_empty", out);
    }

    #[test]
    fn arena_render_single_string_array() {
        let arena = crate::stream_arena::build_stream_arena(
            "[\"ab\"]",
            &crate::PriorityConfig::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        let build = crate::build_priority_order_from_arena(
            &arena,
            &crate::PriorityConfig::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        let mut marks = vec![0u32; build.total_nodes];
        let out = render_arena_with_marks(
            &build,
            10,
            &mut marks,
            1,
            &crate::RenderConfig {
                template: crate::OutputTemplate::Json,
                indent_unit: "  ".to_string(),
                space: " ".to_string(),
                newline: "\n".to_string(),
            },
        );
        assert_snapshot!("arena_render_single", out);
    }
}
