use serde::Serialize;

use crate::order::{ObjectType, PriorityOrder, ROOT_PQ_ID, RankedNode};

#[derive(Serialize)]
struct CountsDbg {
    total_nodes: usize,
    included: usize,
}

#[derive(Serialize)]
struct BudgetsDbg {
    bytes: Option<usize>,
    chars: Option<usize>,
    lines: Option<usize>,
}

#[derive(Serialize)]
pub(crate) struct DumpDbg<'a> {
    root: NodeDbg,
    counts: CountsDbg,
    template: &'a str,
    input_format: &'a str,
    budgets: BudgetsDbg,
}

#[derive(Serialize)]
struct MetricsDbg {
    #[serde(skip_serializing_if = "Option::is_none")]
    array_len: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    object_len: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    string_len: Option<usize>,
    #[allow(
        clippy::trivially_copy_pass_by_ref,
        reason = "serde skip_serializing_if expects a &T predicate signature"
    )]
    #[serde(skip_serializing_if = "is_false")]
    string_truncated: bool,
}

#[allow(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde skip_serializing_if expects a &T predicate signature"
)]
fn is_false(b: &bool) -> bool {
    !*b
}

#[derive(Serialize)]
struct NodeDbg {
    id: usize,
    kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    key_in_object: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    index_in_parent_array: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    string_preview: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    atomic_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fileset_root: Option<bool>,
    metrics: MetricsDbg,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    children: Vec<NodeDbg>,
}

fn template_str_for_root(
    order: &PriorityOrder,
    cfg: &crate::RenderConfig,
) -> &'static str {
    use crate::serialization::types::OutputTemplate as T;
    if order.object_type.get(ROOT_PQ_ID) == Some(&ObjectType::Fileset) {
        // In filesets root, per-file templates may vary under Auto; report "auto".
        return "auto";
    }
    match cfg.template {
        T::Json => "json",
        T::Pseudo => "pseudo",
        T::Js => "js",
        T::Yaml => "yaml",
        T::Text => "text",
        T::Auto => match cfg.style {
            crate::serialization::types::Style::Strict => "json",
            crate::serialization::types::Style::Default => "pseudo",
            crate::serialization::types::Style::Detailed => "js",
        },
    }
}

fn kind_str(node: &RankedNode, atomic_token: Option<&str>) -> String {
    match node {
        RankedNode::Array { .. } => "array".into(),
        RankedNode::Object { .. } => "object".into(),
        RankedNode::SplittableLeaf { .. } => "string".into(),
        RankedNode::LeafPart { .. } => "string-part".into(),
        RankedNode::AtomicLeaf { .. } => match atomic_token {
            Some("null") => "null".into(),
            Some("true") | Some("false") => "bool".into(),
            Some(_) => "number".into(),
            None => "atomic".into(),
        },
    }
}

fn make_metrics(order: &PriorityOrder, id: usize) -> MetricsDbg {
    let m = &order.metrics[id];
    MetricsDbg {
        array_len: m.array_len,
        object_len: m.object_len,
        string_len: m.string_len,
        string_truncated: m.string_truncated,
    }
}

fn string_preview(value: &str) -> String {
    // Show a small, grapheme-aware prefix to aid debugging.
    let prefix = crate::utils::text::take_n_graphemes(value, 32);
    if prefix.len() < value.len() {
        format!("{prefix}…")
    } else {
        prefix
    }
}

#[allow(
    clippy::cognitive_complexity,
    reason = "Pruned tree emission keeps branching in one place for clarity"
)]
fn build_node(
    order: &PriorityOrder,
    id: usize,
    inclusion_flags: &[u32],
    render_id: u32,
    include_count: &mut usize,
) -> NodeDbg {
    let rn = &order.nodes[id];
    let key_in_object =
        rn.key_in_object().map(std::string::ToString::to_string);
    let index_in_parent_array = order.index_in_parent_array[id];
    let fileset_root = if id == ROOT_PQ_ID
        && order.object_type.get(id) == Some(&ObjectType::Fileset)
    {
        Some(true)
    } else {
        None
    };

    // Count only renderable nodes (skip string parts).
    let renderable = !matches!(rn, RankedNode::LeafPart { .. });
    if renderable {
        *include_count += 1;
    }

    // Leaf handling and children traversal
    let (string_preview_opt, atomic_token_opt, children): (
        Option<String>,
        Option<String>,
        Vec<NodeDbg>,
    ) = match rn {
        RankedNode::SplittableLeaf { value, .. } => {
            (Some(string_preview(value)), None, Vec::new())
        }
        RankedNode::AtomicLeaf { token, .. } => {
            (None, Some(token.clone()), Vec::new())
        }
        RankedNode::LeafPart { .. } => (None, None, Vec::new()),
        RankedNode::Array { .. } | RankedNode::Object { .. } => {
            let mut kids = Vec::new();
            if let Some(ch) = order.children.get(id) {
                for &cid in ch.iter() {
                    let cid_usize = cid.0;
                    if inclusion_flags[cid_usize] != render_id {
                        continue;
                    }
                    // Skip synthetic string parts in debug tree to match render
                    if matches!(
                        order.nodes[cid_usize],
                        RankedNode::LeafPart { .. }
                    ) {
                        continue;
                    }
                    kids.push(build_node(
                        order,
                        cid_usize,
                        inclusion_flags,
                        render_id,
                        include_count,
                    ));
                }
            }
            (None, None, kids)
        }
    };

    let atomic_token_ref = atomic_token_opt.as_deref();
    NodeDbg {
        id,
        kind: kind_str(rn, atomic_token_ref),
        key_in_object,
        index_in_parent_array,
        string_preview: string_preview_opt,
        atomic_token: atomic_token_opt,
        fileset_root,
        metrics: make_metrics(order, id),
        children,
    }
}

pub(crate) fn build_render_debug_json(
    order: &PriorityOrder,
    inclusion_flags: &[u32],
    render_id: u32,
    cfg: &crate::RenderConfig,
    budgets: crate::Budgets,
    input_format: &str,
) -> String {
    let mut included = 0usize;
    let root = build_node(
        order,
        ROOT_PQ_ID,
        inclusion_flags,
        render_id,
        &mut included,
    );
    let dump = DumpDbg {
        root,
        counts: CountsDbg {
            total_nodes: order.total_nodes,
            included,
        },
        template: template_str_for_root(order, cfg),
        input_format,
        budgets: BudgetsDbg {
            bytes: budgets.byte_budget,
            chars: budgets.char_budget,
            lines: budgets.line_budget,
        },
    };
    serde_json::to_string_pretty(&dump).unwrap_or_else(|_| "{}".to_string())
}
