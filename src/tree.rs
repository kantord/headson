use anyhow::Result;


use crate::queue::{NodeKind, PQBuild, NodeMetrics};
use crate::{OutputTemplate, RenderConfig};
use crate::render::{ArrayCtx, ObjectCtx, render_array, render_object};
use serde_json::Number;
use unicode_segmentation::UnicodeSegmentation;
 

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TreeKind {
    Array,
    String,
    Object,
    Number,
    Bool,
    Null,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TreeNode {
    pub id: usize,
    pub kind: TreeKind,
    pub value: Option<String>,
    pub index_in_parent: Option<usize>,
    pub key_in_parent: Option<String>,
    pub number_value: Option<Number>,
    pub bool_value: Option<bool>,
    pub children: Vec<TreeNode>,
    pub omitted_items: Option<usize>,
}

impl TreeNode {
    pub fn serialize(&self, config: &RenderConfig) -> String {
        self.serialize_with_depth(config, 0)
    }

    fn serialize_with_depth(&self, config: &RenderConfig, depth: usize) -> String {
        match self.kind {
            TreeKind::Array => self.serialize_array(config, depth),
            TreeKind::String => self.serialize_string(config.template),
            TreeKind::Object => self.serialize_object(config, depth),
            TreeKind::Number => self.serialize_number(),
            TreeKind::Bool => self.serialize_bool(),
            TreeKind::Null => self.serialize_null(),
        }
    }

    fn indent(depth: usize, unit: &str) -> String { unit.repeat(depth) }

    

    fn serialize_array(&self, config: &RenderConfig, depth: usize) -> String {
        // Empty arrays:
        // - truly empty (no original items): always []
        // - truncated to show zero items (omitted_items > 0): show template-specific marker with brackets
        if self.children.is_empty() {
            if let Some(omitted) = self.omitted_items {
                if omitted > 0 {
                    let ctx = ArrayCtx { children: vec![], children_len: 0, omitted, indent0: Self::indent(depth, &config.indent_unit), indent1: Self::indent(depth + 1, &config.indent_unit) };
                    return render_array(config.template, &ctx);
                }
            }
            return "[]".to_string();
        }
        let mut children: Vec<(usize, String)> = Vec::with_capacity(self.children.len());
        let ind = Self::indent(depth + 1, &config.indent_unit);
        for (i, c) in self.children.iter().enumerate() {
            let rendered = c.serialize_with_depth(config, depth + 1);
            if rendered.contains('\n') {
                children.push((i, rendered));
            } else {
                children.push((i, format!("{}{}", ind, rendered)));
            }
        }
        let ctx = ArrayCtx { children_len: children.len(), children, omitted: self.omitted_items.unwrap_or(0), indent0: Self::indent(depth, &config.indent_unit), indent1: ind };
        render_array(config.template, &ctx)
    }

    fn serialize_string(&self, _template: OutputTemplate) -> String {
        if let Some(omitted) = self.omitted_items {
            if omitted > 0 {
                let full = self.value.as_deref().unwrap_or("");
                let keep_n = self.children.len();
                // Build kept prefix by taking first N graphemes from the parent string
                let mut kept = String::new();
                for (i, g) in UnicodeSegmentation::graphemes(full, true).enumerate() {
                    if i >= keep_n { break; }
                    kept.push_str(g);
                }
                let truncated = format!("{}…", kept);
                return serde_json::to_string(&truncated).unwrap_or_else(|_| format!("\"{}…\"", kept));
            }
        }
        let v = self.value.clone().unwrap_or_default();
        serde_json::to_string(&v).unwrap_or_else(|_| format!("\"{}\"", v))
    }

    fn serialize_object(&self, config: &RenderConfig, depth: usize) -> String {
        // Empty objects:
        // - truly empty: {}
        // - truncated to zero visible properties (omitted_items > 0): show marker per template
        if self.children.is_empty() {
            if let Some(omitted) = self.omitted_items {
                if omitted > 0 {
                    let ctx = ObjectCtx { children: vec![], children_len: 0, omitted, indent0: Self::indent(depth, &config.indent_unit), indent1: Self::indent(depth + 1, &config.indent_unit), sp: config.space.clone() };
                    return render_object(config.template, &ctx);
                }
            }
            return "{}".to_string();
        }
        let mut children: Vec<(usize, (String, String))> = Vec::with_capacity(self.children.len());
        let ind = Self::indent(depth + 1, &config.indent_unit);
        for (i, c) in self.children.iter().enumerate() {
            let raw_key = c.key_in_parent.clone().unwrap_or_default();
            // Pre-escape as a JSON string (including quotes) so templates can insert as-is
            let key = serde_json::to_string(&raw_key).unwrap_or_else(|_| format!("\"{}\"", raw_key));
            let mut val = c.serialize_with_depth(config, depth + 1);
            if val.starts_with(&ind) {
                val = val[ind.len()..].to_string();
            }
            children.push((i, (key, val)));
        }
        let ctx = ObjectCtx { children_len: children.len(), children, omitted: self.omitted_items.unwrap_or(0), indent0: Self::indent(depth, &config.indent_unit), indent1: ind, sp: config.space.clone() };
        render_object(config.template, &ctx)
    }

    fn serialize_number(&self) -> String {
        if let Some(ref n) = self.number_value { n.to_string() } else { "0".to_string() }
    }
    fn serialize_bool(&self) -> String {
        if let Some(b) = self.bool_value { if b { "true".to_string() } else { "false".to_string() } } else { "false".to_string() }
    }
    fn serialize_null(&self) -> String { "null".to_string() }

    
}

pub fn build_tree(pq_build: &PQBuild, budget: usize) -> Result<TreeNode> {
    // Fallback wrapper that builds with a fresh mark vector each time.
    let mut marks = vec![0u32; pq_build.total_nodes];
    build_tree_with_marks(pq_build, budget, &mut marks, 1, false)
}

pub(crate) fn build_tree_with_marks(
    pq_build: &PQBuild,
    budget: usize,
    marks: &mut Vec<u32>,
    mark_gen: u32,
    profile: bool,
) -> Result<TreeNode> {
    let t_all_start = std::time::Instant::now();
    let metrics: &Vec<NodeMetrics> = &pq_build.metrics;
    if marks.len() < pq_build.total_nodes { marks.resize(pq_build.total_nodes, 0); }
    // Mark included nodes (order_index < budget) and their ancestors using generation marks
    let t_mark = std::time::Instant::now();
    let mut stack: Vec<usize> = Vec::new();
    for (id, &ord) in pq_build.order_index.iter().enumerate() {
        if ord < budget {
            if marks[id] != mark_gen { marks[id] = mark_gen; stack.push(id); }
        }
    }
    while let Some(id) = stack.pop() {
        if let Some(parent) = pq_build.parent_of[id] {
            if marks[parent] != mark_gen { marks[parent] = mark_gen; stack.push(parent); }
        }
    }
    let mark_ms = (std::time::Instant::now() - t_mark).as_millis();

    // Count included nodes for reporting
    let mut included_count = 0usize;
    for id in 0..pq_build.total_nodes { if marks[id] == mark_gen { included_count += 1; } }

    // Identify root (no parent)
    let root_id = (0..pq_build.total_nodes)
        .find(|&id| pq_build.parent_of[id].is_none())
        .ok_or_else(|| anyhow::anyhow!("no root in queue"))?;

    fn to_tree(
        id: usize,
        pq_build: &PQBuild,
        marks: &Vec<u32>,
        mark_gen: u32,
        metrics: &Vec<NodeMetrics>,
        nodes_built: &mut usize,
        edges_kept: &mut usize,
        depth: usize,
        max_depth: &mut usize,
    ) -> TreeNode {
        let it = &pq_build.id_to_item[id];
        let kind = match it.kind {
            NodeKind::Array => TreeKind::Array,
            NodeKind::String => TreeKind::String,
            NodeKind::Object => TreeKind::Object,
            NodeKind::Number => TreeKind::Number,
            NodeKind::Bool => TreeKind::Bool,
            NodeKind::Null => TreeKind::Null,
        };
        *nodes_built += 1;
        if depth > *max_depth { *max_depth = depth; }
        let mut children_nodes: Vec<TreeNode> = Vec::new();
        if let Some(kids_ids) = pq_build.children_of.get(id) {
            for &cid in kids_ids {
                if marks[cid] == mark_gen {
                    *edges_kept += 1;
                    let child = to_tree(cid, pq_build, marks, mark_gen, metrics, nodes_built, edges_kept, depth + 1, max_depth);
                    children_nodes.push(child);
                }
            }
        }
        let kept = children_nodes.len();
        let omitted_items = match it.kind {
            NodeKind::Array => metrics[id].array_len.and_then(|orig| if orig > kept { Some(orig - kept) } else { None }),
            NodeKind::String => metrics[id].string_len.and_then(|orig| if orig > kept { Some(orig - kept) } else { None }),
            NodeKind::Object => metrics[id].object_len.and_then(|orig| if orig > kept { Some(orig - kept) } else { None }),
            _ => None,
        };
        let number_value = if let NodeKind::Number = it.kind { it.number_value.clone() } else { None };
        let bool_value = if let NodeKind::Bool = it.kind { it.bool_value } else { None };
        let string_value = if let NodeKind::String = it.kind { it.string_value.clone() } else { None };
        TreeNode {
            id,
            kind,
            value: string_value,
            index_in_parent: it.index_in_array,
            key_in_parent: it.key_in_object.clone(),
            number_value,
            bool_value,
            children: children_nodes,
            omitted_items,
        }
    }

    let t_tree = std::time::Instant::now();
    let mut nodes_built = 0usize;
    let mut max_depth = 0usize;
    let mut edges_kept = 0usize;
    let tree = to_tree(root_id, pq_build, marks, mark_gen, metrics, &mut nodes_built, &mut edges_kept, 0, &mut max_depth);
    let tree_ms = (std::time::Instant::now() - t_tree).as_millis();
    if profile {
        let total_ms = (std::time::Instant::now() - t_all_start).as_millis();
        eprintln!(
            "build_tree: mark={}ms, build={}ms (included={}, nodes={}, edges={}, max_depth={}), total={}ms",
            mark_ms, tree_ms, included_count, nodes_built, edges_kept, max_depth, total_ms
        );
    }
    Ok(tree)
}

// no-op: previously used to remove quotes from JSON-escaped keys; now keys are
// pre-escaped with quotes and inserted directly by templates.

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_snapshot;
    use serde_json::Value;

    #[test]
    fn build_tree_empty_array() {
        let value: Value = serde_json::from_str("[]").unwrap();
        let build = crate::build_priority_queue(&value).unwrap();
        let tree = build_tree(&build, 10).unwrap();
        use crate::RenderConfig; use crate::OutputTemplate;
        assert_snapshot!("build_tree_empty", tree.serialize(&RenderConfig{ template: OutputTemplate::Json, indent_unit: "  ".to_string(), space: " ".to_string(), profile: false }));
    }

    #[test]
    fn build_tree_single_string_array() {
        let value: Value = serde_json::from_str("[\"ab\"]").unwrap();
        let build = crate::build_priority_queue(&value).unwrap();
        let tree = build_tree(&build, 10).unwrap();
        use crate::RenderConfig; use crate::OutputTemplate;
        assert_snapshot!("build_tree_single", tree.serialize(&RenderConfig{ template: OutputTemplate::Json, indent_unit: "  ".to_string(), space: " ".to_string(), profile: false }));
    }
}
