use anyhow::Result;
use crate::queue::{NodeKind, PQBuild, NodeMetrics};
use crate::render::{ArrayCtx, ObjectCtx, render_array, render_object};
use unicode_segmentation::UnicodeSegmentation;

fn indent(depth: usize, unit: &str) -> String { unit.repeat(depth) }

// Direct rendering from arena + marks, no TreeNode allocation.
pub(crate) fn render_arena_with_marks(
    pq_build: &PQBuild,
    budget: usize,
    marks: &mut Vec<u32>,
    mark_gen: u32,
    config: &crate::RenderConfig,
    profile: bool,
) -> Result<String> {
    let t_all_start = std::time::Instant::now();
    if marks.len() < pq_build.total_nodes { marks.resize(pq_build.total_nodes, 0); }
    // Mark included nodes (order_index < budget) and their ancestors using generation marks
    let t_mark = std::time::Instant::now();
    let mut stack: Vec<usize> = Vec::new();
    let k = budget.min(pq_build.total_nodes);
    // Mark first k nodes by order directly using ids_by_order (O(k))
    for &id in pq_build.ids_by_order.iter().take(k) {
        if marks[id] != mark_gen { marks[id] = mark_gen; stack.push(id); }
    }
    while let Some(id) = stack.pop() {
        if let Some(parent) = pq_build.parent_of[id] {
            if marks[parent] != mark_gen { marks[parent] = mark_gen; stack.push(parent); }
        }
    }
    let mark_ms = (std::time::Instant::now() - t_mark).as_millis();

    // Identify root (no parent)
    let root_id = (0..pq_build.total_nodes)
        .find(|&id| pq_build.parent_of[id].is_none())
        .ok_or_else(|| anyhow::anyhow!("no root in queue"))?;

    // Helpers for omitted counts
    fn omitted_for(id: usize, kind: &NodeKind, kept: usize, metrics: &Vec<NodeMetrics>) -> Option<usize> {
        match kind {
            NodeKind::Array => metrics[id].array_len.and_then(|orig| if orig > kept { Some(orig - kept) } else { None }),
            NodeKind::String => metrics[id].string_len.and_then(|orig| if orig > kept { Some(orig - kept) } else { None }),
            NodeKind::Object => metrics[id].object_len.and_then(|orig| if orig > kept { Some(orig - kept) } else { None }),
            _ => None,
        }
    }

    // Recursive serialization straight from arena
    fn serialize_node(
        id: usize,
        pq: &PQBuild,
        marks: &Vec<u32>,
        mark_gen: u32,
        cfg: &crate::RenderConfig,
        depth: usize,
        nodes_built: &mut usize,
        max_depth: &mut usize,
    ) -> String {
        *nodes_built += 1;
        if depth > *max_depth { *max_depth = depth; }
        let it = &pq.id_to_item[id];
        match it.kind {
            NodeKind::Array => {
                // Collect kept children
                let mut children_pairs: Vec<(usize, String)> = Vec::new();
                let mut kept = 0usize;
                if let Some(kids) = pq.children_of.get(id) {
                    for (i, &cid) in kids.iter().enumerate() {
                        if marks[cid] == mark_gen {
                            kept += 1;
                            let rendered = serialize_node(cid, pq, marks, mark_gen, cfg, depth + 1, nodes_built, max_depth);
                            let ind = indent(depth + 1, &cfg.indent_unit);
                            if rendered.contains('\n') { children_pairs.push((i, rendered)); }
                            else { children_pairs.push((i, format!("{}{}", ind, rendered))); }
                        }
                    }
                }
                let omitted = omitted_for(id, &it.kind, kept, &pq.metrics).unwrap_or(0);
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
            NodeKind::Object => {
                let mut children_pairs: Vec<(usize, (String, String))> = Vec::new();
                let mut kept = 0usize;
                let ind = indent(depth + 1, &cfg.indent_unit);
                if let Some(kids) = pq.children_of.get(id) {
                    for (i, &cid) in kids.iter().enumerate() {
                        if marks[cid] == mark_gen {
                            kept += 1;
                            let child = &pq.id_to_item[cid];
                            let raw_key = child.key_in_object.clone().unwrap_or_default();
                            let key = serde_json::to_string(&raw_key).unwrap_or_else(|_| format!("\"{}\"", raw_key));
                            let mut val = serialize_node(cid, pq, marks, mark_gen, cfg, depth + 1, nodes_built, max_depth);
                            if val.starts_with(&ind) { val = val[ind.len()..].to_string(); }
                            children_pairs.push((i, (key, val)));
                        }
                    }
                }
                let omitted = omitted_for(id, &it.kind, kept, &pq.metrics).unwrap_or(0);
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
            NodeKind::String => {
                // Truncated? use children count to slice grapheme prefix
                let mut kept = 0usize;
                if let Some(kids) = pq.children_of.get(id) {
                    for &cid in kids { if marks[cid] == mark_gen { kept += 1; } }
                }
                let omitted = omitted_for(id, &it.kind, kept, &pq.metrics).unwrap_or(0);
                let full = it.string_value.clone().unwrap_or_default();
                if omitted > 0 {
                    let mut prefix = String::new();
                    for (i, g) in UnicodeSegmentation::graphemes(full.as_str(), true).enumerate() {
                        if i >= kept { break; }
                        prefix.push_str(g);
                    }
                    let truncated = format!("{}…", prefix);
                    serde_json::to_string(&truncated).unwrap_or_else(|_| format!("\"{}…\"", prefix))
                } else {
                    serde_json::to_string(&full).unwrap_or_else(|_| format!("\"{}\"", full))
                }
            }
            NodeKind::Number => it.number_value.clone().map(|n| n.to_string()).unwrap_or_else(|| "0".to_string()),
            NodeKind::Bool => it.bool_value.map(|b| if b { "true".to_string() } else { "false".to_string() }).unwrap_or_else(|| "false".to_string()),
            NodeKind::Null => "null".to_string(),
        }
    }

    let t_render = std::time::Instant::now();
    let mut nodes_built = 0usize;
    let mut max_depth = 0usize;
    let out = serialize_node(root_id, pq_build, marks, mark_gen, config, 0, &mut nodes_built, &mut max_depth);
    let render_ms = (std::time::Instant::now() - t_render).as_millis();
    if profile {
        let total_ms = (std::time::Instant::now() - t_all_start).as_millis();
        eprintln!(
            "arena_render: mark={}ms, render={}ms (nodes={}, max_depth={}), total={}ms",
            mark_ms, render_ms, nodes_built, max_depth, total_ms
        );
    }
    Ok(out)
}

// no-op: previously used to remove quotes from JSON-escaped keys; now keys are
// pre-escaped with quotes and inserted directly by templates.

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_snapshot;
    use serde_json::Value;

    #[test]
    fn arena_render_empty_array() {
        let value: Value = serde_json::from_str("[]").unwrap();
        let build = crate::build_priority_queue(&value).unwrap();
        let mut marks = vec![0u32; build.total_nodes];
        let out = render_arena_with_marks(&build, 10, &mut marks, 1, &crate::RenderConfig{ template: crate::OutputTemplate::Json, indent_unit: "  ".to_string(), space: " ".to_string(), profile: false }, false).unwrap();
        assert_snapshot!("arena_render_empty", out);
    }

    #[test]
    fn arena_render_single_string_array() {
        let value: Value = serde_json::from_str("[\"ab\"]").unwrap();
        let build = crate::build_priority_queue(&value).unwrap();
        let mut marks = vec![0u32; build.total_nodes];
        let out = render_arena_with_marks(&build, 10, &mut marks, 1, &crate::RenderConfig{ template: crate::OutputTemplate::Json, indent_unit: "  ".to_string(), space: " ".to_string(), profile: false }, false).unwrap();
        assert_snapshot!("arena_render_single", out);
    }
}
