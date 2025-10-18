use anyhow::Result;
use priority_queue::PriorityQueue;
 

use crate::queue::{NodeKind, QueueItem, PQBuild, NodeMetrics};
use std::cell::RefCell;
use crate::OutputTemplate;

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
    pub number_value: Option<f64>,
    pub bool_value: Option<bool>,
    pub children: Vec<TreeNode>,
    pub omitted_items: Option<usize>,
    cached: RefCell<Option<String>>,
}

impl TreeNode {
    pub fn serialize(&self, template: OutputTemplate) -> String {
        if let Some(s) = self.cached.borrow().as_ref() { return s.clone(); }
        let s = self.serialize_with_depth(template, 0);
        *self.cached.borrow_mut() = Some(s.clone());
        s
    }

    fn serialize_with_depth(&self, template: OutputTemplate, depth: usize) -> String {
        match self.kind {
            TreeKind::Array => self.serialize_array(template, depth),
            TreeKind::String => self.serialize_string(template),
            TreeKind::Object => self.serialize_object(template, depth),
            TreeKind::Number => self.serialize_number(),
            TreeKind::Bool => self.serialize_bool(),
            TreeKind::Null => self.serialize_null(),
        }
    }

    fn indent(depth: usize) -> String { "  ".repeat(depth) }

    pub fn reset_cache(&self) {
        *self.cached.borrow_mut() = None;
        for child in &self.children { child.reset_cache(); }
    }

    fn serialize_array(&self, template: OutputTemplate, depth: usize) -> String {
        // Special truncation marker when array has single empty-string child
        if self.children.len() == 1 {
            let child = &self.children[0];
            if matches!(child.kind, TreeKind::String) && child.value.as_deref() == Some("") {
                return match template {
                    OutputTemplate::Json => format!("{}[\n{}\n{}]", Self::indent(depth), Self::indent(depth+1), Self::indent(depth)),
                    OutputTemplate::Pseudo => format!("{}[\n{}…\n{}]", Self::indent(depth), Self::indent(depth+1), Self::indent(depth)),
                    OutputTemplate::Js => format!("{}[\n{}/* 1 more item */\n{}]", Self::indent(depth), Self::indent(depth+1), Self::indent(depth)),
                };
            }
        }

        let mut out = String::new();
        out.push_str(&format!("{}[\n", Self::indent(depth)));
        for (i, child) in self.children.iter().enumerate() {
            let rendered = child.serialize_with_depth(template, depth + 1);
            if rendered.contains('\n') {
                out.push_str(&rendered);
            } else {
                out.push_str(&format!("{}{}", Self::indent(depth + 1), rendered));
            }
            if i + 1 < self.children.len() { out.push(','); }
            out.push('\n');
        }
        // If items were omitted by PQ truncation, append a marker for pseudo/js
        if let Some(omitted) = self.omitted_items {
            if omitted > 0 {
                match template {
                    OutputTemplate::Json => {}
                    OutputTemplate::Pseudo => {
                        out.push_str(&format!("{}…\n", Self::indent(depth + 1)));
                    }
                    OutputTemplate::Js => {
                        out.push_str(&format!("{}/* {} more items */\n", Self::indent(depth + 1), omitted));
                    }
                }
            }
        }
        out.push_str(&format!("{}]", Self::indent(depth)));
        out
    }

    fn serialize_string(&self, _template: OutputTemplate) -> String {
        // If string was truncated by PQ, render the kept prefix + ellipsis inside quotes
        if let Some(omitted) = self.omitted_items {
            if omitted > 0 {
                let mut kept = String::new();
                for child in &self.children {
                    if matches!(child.kind, TreeKind::String) {
                        kept.push_str(child.value.as_deref().unwrap_or(""));
                    }
                }
                return format!("\"{}…\"", kept);
            }
        }
        let v = self.value.clone().unwrap_or_default();
        format!("\"{}\"", v)
    }

    fn serialize_object(&self, template: OutputTemplate, depth: usize) -> String {
        let mut out = String::new();
        out.push_str(&format!("{}{{\n", Self::indent(depth)));
        for (i, child) in self.children.iter().enumerate() {
            let key = child.key_in_parent.as_deref().unwrap_or("");
            let mut rendered = child.serialize_with_depth(template, depth + 1);
            let first_indent = Self::indent(depth + 1);
            if rendered.starts_with(&first_indent) {
                rendered = rendered[first_indent.len()..].to_string();
            }
            out.push_str(&format!("{}\"{}\": {}", Self::indent(depth + 1), key, rendered));
            if i + 1 < self.children.len() { out.push(','); }
            out.push('\n');
        }
        out.push_str(&format!("{}}}", Self::indent(depth)));
        out
    }

    fn serialize_number(&self) -> String {
        if let Some(n) = self.number_value { format!("{}", n) } else { "0".to_string() }
    }
    fn serialize_bool(&self) -> String {
        if let Some(b) = self.bool_value { if b { "true".to_string() } else { "false".to_string() } } else { "false".to_string() }
    }
    fn serialize_null(&self) -> String { "null".to_string() }

    
}

pub fn build_tree(pq_build: &PQBuild, budget: usize) -> Result<TreeNode> {
    let pq: &PriorityQueue<QueueItem, usize> = &pq_build.pq;
    let metrics: &std::collections::HashMap<usize, NodeMetrics> = &pq_build.metrics;
    // Collect first N by ascending priority (shallower depth first), N = budget for now
    let mut all_desc: Vec<(QueueItem, usize)> = pq.clone().into_sorted_iter().collect();
    all_desc.reverse();
    let items: Vec<QueueItem> = all_desc.into_iter().take(budget).map(|(it, _)| it).collect();

    // Index by id
    #[derive(Clone, Debug)]
    struct Rec {
        kind: NodeKind,
        index: Option<usize>,
        key: Option<String>,
        value: Option<String>,
    }

    let mut map = std::collections::HashMap::<usize, Rec>::new();
    for it in &items {
        let val = match it.kind {
            NodeKind::String => Some(strip_quotes(&it.value_repr)),
            NodeKind::Number => Some(it.value_repr.clone()),
            NodeKind::Bool => Some(it.value_repr.clone()),
            _ => None,
        };
        map.insert(
            it.node_id.0,
            Rec {
                kind: it.kind.clone(),
                index: it.index_in_array,
                key: it.key_in_object.clone(),
                value: val,
            },
        );
    }

    // Build children lists; include string character children so strings can truncate like arrays
    let mut children: std::collections::HashMap<usize, Vec<usize>> = std::collections::HashMap::new();
    for it in &items {
        if let Some(pid) = it.parent_id.0 {
            children.entry(pid).or_default().push(it.node_id.0);
        }
    }

    // Identify root (no parent)
    let root_id = items
        .iter()
        .find(|it| it.parent_id.0.is_none())
        .map(|it| it.node_id.0)
        .ok_or_else(|| anyhow::anyhow!("no root in queue"))?;

    fn to_tree(
        id: usize,
        map: &std::collections::HashMap<usize, Rec>,
        children: &std::collections::HashMap<usize, Vec<usize>>,
        metrics: &std::collections::HashMap<usize, NodeMetrics>,
    ) -> TreeNode {
        let rec = map.get(&id).expect("missing rec");
        let kind = match rec.kind {
            NodeKind::Array => TreeKind::Array,
            NodeKind::String => TreeKind::String,
            NodeKind::Object => TreeKind::Object,
            NodeKind::Number => TreeKind::Number,
            NodeKind::Bool => TreeKind::Bool,
            NodeKind::Null => TreeKind::Null,
        };
        let mut kids_ids = children.get(&id).cloned().unwrap_or_default();
        // Sort by index for array-like children
        kids_ids.sort_by_key(|cid| map.get(cid).and_then(|r| r.index).unwrap_or(usize::MAX));
        let kids = kids_ids
            .into_iter()
            .map(|cid| to_tree(cid, map, children, metrics))
            .collect::<Vec<_>>();
        // Compute omitted items for arrays using PQ metrics
        let omitted_items = match rec.kind {
            NodeKind::Array => {
                if let Some(node_metrics) = metrics.get(&id) {
                    if let Some(orig_len) = node_metrics.array_len {
                        let kept = kids.len();
                        if orig_len > kept { Some(orig_len - kept) } else { None }
                    } else { None }
                } else { None }
            }
            NodeKind::String => {
                if let Some(node_metrics) = metrics.get(&id) {
                    if let Some(orig_len) = node_metrics.string_len {
                        let kept = kids.len();
                        if orig_len > kept { Some(orig_len - kept) } else { None }
                    } else { None }
                } else { None }
            }
            _ => None,
        };
        TreeNode {
            id,
            kind,
            value: match rec.kind { NodeKind::String => rec.value.clone(), _ => None },
            index_in_parent: rec.index,
            key_in_parent: rec.key.clone(),
            number_value: match rec.kind { NodeKind::Number => rec.value.as_deref().and_then(|s| s.parse::<f64>().ok()), _ => None },
            bool_value: match rec.kind { NodeKind::Bool => Some(rec.value.as_deref() == Some("true")), _ => None },
            children: kids,
            omitted_items,
            cached: RefCell::new(None),
        }
    }

    Ok(to_tree(root_id, &map, &children, metrics))
}

fn strip_quotes(s: &str) -> String {
    let mut out = s.to_string();
    if out.starts_with('"') && out.ends_with('"') && out.len() >= 2 {
        out.remove(0);
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::queue::build_priority_queue;
    use insta::assert_snapshot;
    use serde_json::Value;

    #[test]
    fn build_tree_empty_array() {
        let value: Value = serde_json::from_str("[]").unwrap();
        let build = crate::build_priority_queue(&value).unwrap();
        let tree = build_tree(&build, 10).unwrap();
        use crate::OutputTemplate;
        assert_snapshot!("build_tree_empty", tree.serialize(OutputTemplate::Json));
    }

    #[test]
    fn build_tree_single_string_array() {
        let value: Value = serde_json::from_str("[\"ab\"]").unwrap();
        let build = crate::build_priority_queue(&value).unwrap();
        let tree = build_tree(&build, 10).unwrap();
        use crate::OutputTemplate;
        assert_snapshot!("build_tree_single", tree.serialize(OutputTemplate::Json));
    }
}
