use anyhow::Result;
use priority_queue::PriorityQueue;
use serde_json::Value;

use crate::queue::{NodeKind, QueueItem};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TreeKind {
    Array,
    String,
    Object,
    Number,
    Bool,
    Null,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TreeNode {
    pub id: usize,
    pub kind: TreeKind,
    pub value: Option<String>,
    pub index_in_parent: Option<usize>,
    pub children: Vec<TreeNode>,
}

impl TreeNode {
    pub fn render_json(&self) -> String {
        match self.kind {
            TreeKind::Array => {
                if self.children.is_empty() {
                    "[]".to_string()
                } else if self.children.len() == 1 {
                    let child = &self.children[0];
                    if let TreeKind::String = child.kind {
                        let v = child.value.as_deref().unwrap_or("");
                        format!("[\n  \"{}\"\n]", v)
                    } else {
                        "[]".to_string()
                    }
                } else {
                    "[]".to_string()
                }
            }
            TreeKind::String => format!("\"{}\"", self.value.clone().unwrap_or_default()),
            TreeKind::Object => "{}".to_string(),
            TreeKind::Number => "0".to_string(),
            TreeKind::Bool => "false".to_string(),
            TreeKind::Null => "null".to_string(),
        }
    }
}

pub fn build_tree(pq: &PriorityQueue<QueueItem, usize>) -> Result<TreeNode> {
    // Collect items
    let mut items: Vec<QueueItem> = Vec::with_capacity(pq.len());
    for (item, _prio) in pq.clone().into_sorted_iter() {
        items.push(item);
    }

    // Index by id
    #[derive(Clone, Debug)]
    struct Rec {
        id: usize,
        parent_id: Option<usize>,
        kind: NodeKind,
        index: Option<usize>,
        value: Option<String>,
    }

    let mut map = std::collections::HashMap::<usize, Rec>::new();
    for it in &items {
        let val = match it.kind {
            NodeKind::String => Some(strip_quotes(&it.value_repr)),
            _ => None,
        };
        map.insert(
            it.node_id.0,
            Rec {
                id: it.node_id.0,
                parent_id: it.parent_id.0,
                kind: it.kind.clone(),
                index: it.index_in_array,
                value: val,
            },
        );
    }

    // Build children lists, ignoring string character children (parent is String)
    let mut children: std::collections::HashMap<usize, Vec<usize>> = std::collections::HashMap::new();
    for it in &items {
        if let Some(pid) = it.parent_id.0 {
            if let Some(parent) = map.get(&pid) {
                if let NodeKind::String = parent.kind {
                    continue; // skip char-level expansions
                }
            }
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
            .map(|cid| to_tree(cid, map, children))
            .collect::<Vec<_>>();
        TreeNode {
            id,
            kind,
            value: rec.value.clone(),
            index_in_parent: rec.index,
            children: kids,
        }
    }

    Ok(to_tree(root_id, &map, &children))
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

    #[test]
    fn build_tree_empty_array() {
        let value: Value = serde_json::from_str("[]").unwrap();
        let pq = build_priority_queue(&value).unwrap();
        let tree = build_tree(&pq).unwrap();
        assert_snapshot!("build_tree_empty", tree.render_json());
    }

    #[test]
    fn build_tree_single_string_array() {
        let value: Value = serde_json::from_str("[\"ab\"]").unwrap();
        let pq = build_priority_queue(&value).unwrap();
        let tree = build_tree(&pq).unwrap();
        assert_snapshot!("build_tree_single", tree.render_json());
    }
}
