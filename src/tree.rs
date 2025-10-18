use anyhow::Result;


use crate::queue::{NodeKind, PQBuild, NodeMetrics};
use crate::{OutputTemplate, RenderConfig};
use crate::render::{ArrayCtx, ObjectCtx, render_array, render_object};
use serde_json::Number;
 

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
                let mut kept = String::new();
                for child in &self.children {
                    if matches!(child.kind, TreeKind::String) {
                        kept.push_str(child.value.as_deref().unwrap_or(""));
                    }
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
            let key = strip_quotes(&serde_json::to_string(&raw_key).unwrap_or_else(|_| format!("\"{}\"", raw_key)));
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
    let metrics: &std::collections::HashMap<usize, NodeMetrics> = &pq_build.metrics;
    // Include nodes with order_index < budget, plus ensure ancestors are included
    let mut included: std::collections::HashSet<usize> = std::collections::HashSet::new();
    let mut to_process: Vec<usize> = Vec::new();
    for (id, &ord) in &pq_build.order_index {
        if ord < budget { included.insert(*id); to_process.push(*id); }
    }
    while let Some(id) = to_process.pop() {
        if let Some(parent) = pq_build.parent_of.get(&id).copied().flatten() {
            if included.insert(parent) { to_process.push(parent); }
        }
    }

    // Index by id
    #[derive(Clone, Debug)]
    struct Rec {
        kind: NodeKind,
        index: Option<usize>,
        key: Option<String>,
        value: Option<String>,
        number: Option<Number>,
    }

    let mut map = std::collections::HashMap::<usize, Rec>::new();
    for id in &included {
        let it = pq_build.id_to_item.get(id).expect("missing item");
        let val = match it.kind {
            NodeKind::String => Some(strip_quotes(&it.value_repr)),
            NodeKind::Bool => Some(it.value_repr.clone()),
            _ => None,
        };
        let number = if let NodeKind::Number = it.kind {
            serde_json::from_str::<serde_json::Value>(&it.value_repr)
                .ok()
                .and_then(|v| if let serde_json::Value::Number(n) = v { Some(n) } else { None })
        } else { None };
        map.insert(
            it.node_id.0,
            Rec {
                kind: it.kind.clone(),
                index: it.index_in_array,
                key: it.key_in_object.clone(),
                value: val,
                number,
            },
        );
    }

    // Build children lists using arena, filter to included
    let mut children: std::collections::HashMap<usize, Vec<usize>> = std::collections::HashMap::new();
    for (pid, kids) in &pq_build.children_of {
        let kept = kids.iter().copied().filter(|cid| included.contains(cid)).collect::<Vec<_>>();
        if !kept.is_empty() { children.insert(*pid, kept); }
    }

    // Identify root (no parent)
    let root_id = pq_build
        .id_to_item
        .values()
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
        // Compute omitted items for arrays/strings/objects using PQ metrics
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
            NodeKind::Object => {
                if let Some(node_metrics) = metrics.get(&id) {
                    if let Some(orig_len) = node_metrics.object_len {
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
            number_value: match rec.kind { NodeKind::Number => rec.number.clone(), _ => None },
            bool_value: match rec.kind { NodeKind::Bool => Some(rec.value.as_deref() == Some("true")), _ => None },
            children: kids,
            omitted_items,
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
    use insta::assert_snapshot;
    use serde_json::Value;

    #[test]
    fn build_tree_empty_array() {
        let value: Value = serde_json::from_str("[]").unwrap();
        let build = crate::build_priority_queue(&value).unwrap();
        let tree = build_tree(&build, 10).unwrap();
        use crate::RenderConfig; use crate::OutputTemplate;
        assert_snapshot!("build_tree_empty", tree.serialize(&RenderConfig{ template: OutputTemplate::Json, indent_unit: "  ".to_string(), space: " ".to_string() }));
    }

    #[test]
    fn build_tree_single_string_array() {
        let value: Value = serde_json::from_str("[\"ab\"]").unwrap();
        let build = crate::build_priority_queue(&value).unwrap();
        let tree = build_tree(&build, 10).unwrap();
        use crate::RenderConfig; use crate::OutputTemplate;
        assert_snapshot!("build_tree_single", tree.serialize(&RenderConfig{ template: OutputTemplate::Json, indent_unit: "  ".to_string(), space: " ".to_string() }));
    }
}
