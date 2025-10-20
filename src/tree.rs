use crate::order::{NodeKind, PriorityOrder};
use crate::render::{ArrayCtx, ObjectCtx, render_array, render_object};
use anyhow::Result;
use unicode_segmentation::UnicodeSegmentation;

fn indent(depth: usize, unit: &str) -> String {
    unit.repeat(depth)
}

/// Render a budget-limited preview directly from the arena using inclusion marks.
///
/// How marks work
/// - `marks` is a buffer indexed by node id. When `marks[id] == mark_gen`, that
///   node is considered included for the current probe/render. Any other value
///   means the node is excluded.
/// - `mark_gen` is a monotonically increasing generation counter. Each probe
///   increments it, so we can reuse the same `marks` vector without clearing it
///   (an O(N) operation). Setting a node as included is just `marks[id] = mark_gen`.
///
/// Algorithm (two phases per probe)
/// 1) Mark phase (O(k + ancestors)):
///    - Take the first `k` node ids from `ids_by_order` (the global priority
///      order previously built by `build_priority_order_from_arena`). Mark each
///      of those nodes as included.
///    - Walk up parent links to mark ancestors as included too. This guarantees
///      tree integrity (no child appears without its parent in the output).
/// 2) Serialize phase (O(kept_nodes)):
///    - Starting from the root, traverse only into children whose ids are
///      marked with the current `mark_gen`. Serialize each node according to
///      the selected output style. For arrays/objects/strings, compute and print
///      “omitted” counts using `NodeMetrics` (recorded during the order build)
///      to indicate how many items/props/graphemes were not included.
///
/// Integration with search under a budget
/// - Callers (see `find_largest_render_under_budget`) binary-search the `k`
///   value to find the largest render that fits within the character budget.
///   The generation-mark scheme makes repeated probes cheap because we avoid
///   clearing marks and only render the currently included subset.
///
/// Notes
/// - This function does not allocate a tree structure; it renders directly from
///   the arena (`PriorityOrder` + `StreamArena`-derived structures), which keeps
///   memory and CPU overhead low for repeated probes.
pub fn render_arena_with_marks(
    order_build: &PriorityOrder,
    budget: usize,
    marks: &mut Vec<u32>,
    mark_gen: u32,
    config: &crate::RenderConfig,
    profile: bool,
) -> Result<String> {
    let t_all_start = std::time::Instant::now();
    if marks.len() < order_build.total_nodes {
        marks.resize(order_build.total_nodes, 0);
    }
    // Phase 1: Mark the first `k` nodes (ids_by_order[..k]) and all their ancestors
    // using the current generation counter.
    let t_mark = std::time::Instant::now();
    let mut stack: Vec<usize> = Vec::new();
    let k = budget.min(order_build.total_nodes);
    // Mark first k nodes by order directly using ids_by_order (O(k)).
    for &id in order_build.ids_by_order.iter().take(k) {
        if marks[id] != mark_gen {
            marks[id] = mark_gen;
            stack.push(id);
        }
    }
    while let Some(id) = stack.pop() {
        match order_build.parent_of[id] {
            Some(parent) if marks[parent] != mark_gen => {
                marks[parent] = mark_gen;
                stack.push(parent);
            }
            _ => {}
        }
    }
    let mark_ms = t_mark.elapsed().as_millis();

    // Identify root (node with no parent). There should be exactly one.
    let root_id = (0..order_build.total_nodes)
        .find(|&id| order_build.parent_of[id].is_none())
        .ok_or_else(|| anyhow::anyhow!("no root in queue"))?;

    // Phase 2: Use a small scope struct to keep state and reduce parameter lists.
    struct RenderScope<'a> {
        pq: &'a PriorityOrder,
        marks: &'a [u32],
        mark_gen: u32,
        cfg: &'a crate::RenderConfig,
        nodes_built: usize,
        max_depth: usize,
    }

    impl<'a> RenderScope<'a> {
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
                NodeKind::String => {
                    if let Some(orig) = self.pq.metrics[id].string_len {
                        if orig > kept { Some(orig - kept) } else { None }
                    } else if self.pq.metrics[id].string_truncated {
                        Some(1)
                    } else {
                        None
                    }
                }
                NodeKind::Object => {
                    self.pq.metrics[id].object_len.and_then(|orig| {
                        if orig > kept { Some(orig - kept) } else { None }
                    })
                }
                _ => None,
            }
        }

        fn serialize_array(&mut self, id: usize, depth: usize) -> String {
            let cfg = self.cfg;
            let mut children_pairs: Vec<(usize, String)> = Vec::new();
            let mut kept = 0usize;
            if let Some(kids) = self.pq.children_of.get(id) {
                for (i, &cid) in kids.iter().enumerate() {
                    if self.marks[cid] == self.mark_gen {
                        kept += 1;
                        let rendered = self.serialize_node(cid, depth + 1);
                        let ind = indent(depth + 1, &cfg.indent_unit);
                        if rendered.contains('\n') {
                            children_pairs.push((i, rendered));
                        } else {
                            children_pairs
                                .push((i, format!("{ind}{rendered}")));
                        }
                    }
                }
            }
            let it = &self.pq.id_to_item[id];
            let omitted = self.omitted_for(id, &it.kind, kept).unwrap_or(0);
            if kept == 0 && omitted == 0 {
                return "[]".to_string();
            }
            let ctx = ArrayCtx {
                children: children_pairs,
                children_len: kept,
                omitted,
                indent0: indent(depth, &cfg.indent_unit),
                indent1: indent(depth + 1, &cfg.indent_unit),
            };
            render_array(cfg.template, &ctx)
        }

        fn serialize_object(&mut self, id: usize, depth: usize) -> String {
            let cfg = self.cfg;
            let mut children_pairs: Vec<(usize, (String, String))> =
                Vec::new();
            let mut kept = 0usize;
            let ind = indent(depth + 1, &cfg.indent_unit);
            if let Some(kids) = self.pq.children_of.get(id) {
                for (i, &cid) in kids.iter().enumerate() {
                    if self.marks[cid] == self.mark_gen {
                        kept += 1;
                        let child = &self.pq.id_to_item[cid];
                        let raw_key =
                            child.key_in_object.clone().unwrap_or_default();
                        let key = serde_json::to_string(&raw_key)
                            .unwrap_or_else(|_| format!("\"{raw_key}\""));
                        let mut val = self.serialize_node(cid, depth + 1);
                        if val.starts_with(&ind) {
                            val = val[ind.len()..].to_string();
                        }
                        children_pairs.push((i, (key, val)));
                    }
                }
            }
            let it = &self.pq.id_to_item[id];
            let omitted = self.omitted_for(id, &it.kind, kept).unwrap_or(0);
            if kept == 0 && omitted == 0 {
                return "{}".to_string();
            }
            let ctx = ObjectCtx {
                children: children_pairs,
                children_len: kept,
                omitted,
                indent0: indent(depth, &cfg.indent_unit),
                indent1: ind,
                sp: cfg.space.clone(),
            };
            render_object(cfg.template, &ctx)
        }

        fn serialize_string(&mut self, id: usize) -> String {
            let mut kept = 0usize;
            if let Some(kids) = self.pq.children_of.get(id) {
                for &cid in kids {
                    if self.marks[cid] == self.mark_gen {
                        kept += 1;
                    }
                }
            }
            let it = &self.pq.id_to_item[id];
            let omitted = self.omitted_for(id, &it.kind, kept).unwrap_or(0);
            let full = it.string_value.clone().unwrap_or_default();
            if omitted > 0 {
                let mut prefix = String::new();
                for (i, g) in
                    UnicodeSegmentation::graphemes(full.as_str(), true)
                        .enumerate()
                {
                    if i >= kept {
                        break;
                    }
                    prefix.push_str(g);
                }
                let truncated = format!("{prefix}…");
                serde_json::to_string(&truncated)
                    .unwrap_or_else(|_| format!("\"{prefix}…\""))
            } else {
                serde_json::to_string(&full)
                    .unwrap_or_else(|_| format!("\"{full}\""))
            }
        }

        fn serialize_node(&mut self, id: usize, depth: usize) -> String {
            self.nodes_built += 1;
            if depth > self.max_depth {
                self.max_depth = depth;
            }
            let it = &self.pq.id_to_item[id];
            match it.kind {
                NodeKind::Array => self.serialize_array(id, depth),
                NodeKind::Object => self.serialize_object(id, depth),
                NodeKind::String => self.serialize_string(id),
                NodeKind::Number => {
                    if let Some(n) = it.number_value.as_ref() {
                        if let Some(i) = n.as_i64() {
                            return i.to_string();
                        }
                        if let Some(u) = n.as_u64() {
                            return u.to_string();
                        }
                        if let Some(f) = n.as_f64() {
                            if f == 0.0 {
                                return "0.0".to_string();
                            }
                            return n.to_string();
                        }
                    }
                    "0".to_string()
                }
                NodeKind::Bool => it.bool_value.map_or_else(
                    || "false".to_string(),
                    |b| {
                        if b {
                            "true".to_string()
                        } else {
                            "false".to_string()
                        }
                    },
                ),
                NodeKind::Null => "null".to_string(),
            }
        }
    }

    let t_render = std::time::Instant::now();
    let mut scope = RenderScope {
        pq: order_build,
        marks,
        mark_gen,
        cfg: config,
        nodes_built: 0,
        max_depth: 0,
    };
    let out = scope.serialize_node(root_id, 0);
    let render_ms = t_render.elapsed().as_millis();
    if profile {
        let total_ms = t_all_start.elapsed().as_millis();
        eprintln!(
            "arena_render: mark={mark_ms}ms, render={render_ms}ms (nodes={}, max_depth={}), total={total_ms}ms",
            scope.nodes_built, scope.max_depth
        );
    }
    Ok(out)
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
                profile: false,
            },
            false,
        )
        .unwrap();
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
                profile: false,
            },
            false,
        )
        .unwrap();
        assert_snapshot!("arena_render_single", out);
    }
}
